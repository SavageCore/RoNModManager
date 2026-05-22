<script lang="ts">
  import { afterUpdate, createEventDispatcher } from "svelte";
  const dispatch = createEventDispatcher();
  import type { ModInfo } from "$lib/types/modpack";

  export let isVisible = false;
  export let mode: "add" | "update" = "add";
  export let currentVersion: string | null = null;
  export let newVersion: string | null = null;

  let url = "";
  let log: string[] = [];
  let isLoading = false;
  let isValid = false;
  let error = "";
  let logDiv: HTMLDivElement | null;
  let existingUrl: string | null = null;

  function scrollLog() {
    if (logDiv) {
      logDiv.scrollTop = logDiv.scrollHeight;
    }
  }

  afterUpdate(scrollLog);

  function copyLog() {
    navigator.clipboard
      .writeText(log.join("\n"))
      .then(() => {
        // optional feedback
      })
      .catch((err) => {
        console.error("Failed to copy: ", err);
      });
  }

  function saveLog() {
    const now = new Date();
    const pad = (n: number) => n.toString().padStart(2, "0");
    const timestamp = `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}-${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
    const filename = `modpack-log-${timestamp}.txt`;
    const blob = new Blob([log.join("\n")], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = filename;
    a.click();
    URL.revokeObjectURL(url);
  }

  function close() {
    if (mode === "add") url = "";
    log = [];
    isLoading = false;
    isValid = false;
    error = "";
    dispatch("close");
  }

  import {
    addModIoMod,
    downloadModArchive,
    fetchModpackJson,
    fetchModioRemoteInfo,
    fileExists,
    getArchiveRootPath,
    getConfig,
    getInstalledModGroups,
    installLocalMod,
    readManifestForArchive,
    updateConfig,
    updateModSourceUrl,
  } from "$lib/api/commands";
  import { operationStatusStore } from "$lib/stores/operationStatus";
  import { tick } from "svelte";

  $: if (isVisible) {
    (async () => {
      try {
        const config = await getConfig();
        existingUrl = config.modpack_url || null;
        console.log("Existing modpack URL from config:", existingUrl);
      } catch (e) {
        console.warn("Could not retrieve existing modpack URL from config.");
      }
    })();
  }

  async function handleSave() {
    log = ["Validating URL..."];
    isLoading = true;
    error = "";
    isValid = false;
    let hadError = false;
    let data;
    const archiveRootPath = await getArchiveRootPath();
    try {
      // Basic URL validation
      let modpackUrl: string | null = url;
      if (mode === "update") {
        console.log(
          "Update mode: Ignoring input URL and using existing config value",
        );
        // Always use config value for update
        const config = await getConfig();
        modpackUrl = config.modpack_url;
        url = modpackUrl || "";
      }

      if (modpackUrl) {
        log.push(`Fetching modpack from URL: ${modpackUrl} ...`);
        try {
          data = await fetchModpackJson(modpackUrl);
        } catch (fetchErr: any) {
          log.push(
            `Failed to fetch modpack: ${fetchErr.message || String(fetchErr)}`,
          );
          await tick();
          error = `Failed to fetch modpack: ${fetchErr.message || String(fetchErr)}`;
          isLoading = false;
          return;
        }
      }

      // Update config with modpack_url and modpack_version
      try {
        await updateConfig({ modpack_url: url, modpack_version: data.version });
        log.push("Saved modpack URL and version to config.");
        await tick();
      } catch (e) {
        log.push("Warning: Could not update config with modpack URL/version.");
        await tick();
      }

      const modEntries = Object.entries(data.mods);
      log.push("Checking mods folder...");
      await tick();
      if (modEntries.length === 0) {
        log.push("No mods found in modpack.");
        error = "No mods found in modpack.";
        isLoading = false;
        return;
      }
      log.push("Mods folder found.");
      await tick();
      // Get base URL for self-hosted downloads
      const baseUrl = url.replace(/\/[^/]*$/, "");

      // Process all mods in the mods object
      // Define the type for modInfo based on expected modpack.json structure
      for (const [modFile, modInfo] of modEntries as [string, ModInfo][]) {
        const src = modInfo.source_url || "";

        // mod.io mods are always direct-downloaded, never self-hosted
        if (src.toLowerCase().includes("mod.io")) {
          log.push(`Installing mod.io mod: ${modFile}...`);
          log = log;
          await tick();
          try {
            const remoteInfo = await fetchModioRemoteInfo(src);
            let manifest = null;
            try {
              manifest = await readManifestForArchive(remoteInfo.archive_name);
            } catch {}
            const archivePath = `${archiveRootPath}/${remoteInfo.archive_name}`;
            if (
              remoteInfo.remote_md5 &&
              manifest?.content_hash === remoteInfo.remote_md5 &&
              (await fileExists(archivePath))
            ) {
              log.push(`Already up-to-date, skipping download.`);
              log = log;
              await tick();
            } else {
              const result = await addModIoMod(src);
              log.push(`Installed '${result.name}' from mod.io.`);
              log = log;
              await tick();
            }
          } catch (modErr: any) {
            log.push(
              `Error installing mod.io mod: ${modErr.message || String(modErr)}`,
            );
            log = log;
            await tick();
            error = modErr.message || String(modErr);
            hadError = true;
          }
          continue;
        }

        log.push(`Processing mod: ${modFile} ...`);
        log = log;
        // Download from self-hosted server or Nexus
        // Manifest hash check logic
        let manifestHashMatched = false;
        try {
          let manifest = null;
          try {
            manifest = await readManifestForArchive(modFile);
          } catch (err: any) {
            log.push(
              `Could not read manifest for ${modFile} (backend error: ${err && err.message ? err.message : String(err)})`,
            );
            log = log;
            await tick();
          }
          const archivePath = `${archiveRootPath}/${modFile}`;
          if (
            manifest &&
            manifest.content_hash &&
            modInfo.content_hash &&
            manifest.content_hash === modInfo.content_hash
          ) {
            // Check file existence using backend Tauri command
            let fileExistsResult = false;
            try {
              fileExistsResult = await fileExists(archivePath);
            } catch {
              log.push(
                `Error checking file existence for ${archivePath}. Will attempt download. (Error details hidden)`,
              );
              log = log;
              await tick();
            }
            if (fileExistsResult) {
              log.push("Found hash of archive in local manifest");
              log.push("Hash matches modpack");
              log.push("Skipping download");
              log.push("Installing...");
              log = log;
              await tick();
              try {
                await installLocalMod(archivePath);
                log.push("Installed");
                log = log;
                await tick();
              } catch (installErr: any) {
                log.push(
                  `Error installing archive: ${installErr.message || String(installErr)}`,
                );
                log = log;
                await tick();
                error = installErr.message || String(installErr);
                hadError = true;
              }
              manifestHashMatched = true;
            } else {
              log.push(
                `Hash matches modpack but archive not found at expected path: ${archivePath}`,
              );
              log = log;
              await tick();
            }
          } else {
            log.push(
              `File does not exist or hash mismatch (manifest hash: ${manifest && manifest.content_hash ? manifest.content_hash : "N/A"}, modpack hash: ${modInfo.content_hash ? modInfo.content_hash : "N/A"})`,
            );
            log = log;
            await tick();
          }
        } catch {}
        if (!manifestHashMatched) {
          const downloadUrl = `${baseUrl}/mods/${encodeURIComponent(modFile)}`;
          log.push(`Downloading...`);
          log = log;
          await tick();
          operationStatusStore.setTemporaryMessage(
            `Downloading ${modFile} ...`,
          );
          try {
            await downloadModArchive(downloadUrl, modFile);
            log.push(`Downloaded '${modFile}'.`);
            log = log;
            await tick();
            const archivePath = `${archiveRootPath}/${modFile}`;
            // log.push(`Checking file exists: ${archivePath}`); // REMOVE per requirements
            // log = log;
            // await tick();
            try {
              await installLocalMod(archivePath);
              log.push(`Installed '${modFile}'.`);
              log = log;
              await tick();
              try {
                await updateModSourceUrl(modFile, src);
                // log.push(`Set source_url for '${modFile}'.`);
                // log = log;
                // await tick();
              } catch (setUrlErr: any) {
                log.push(
                  `Warning: Failed to set source_url: ${setUrlErr.message || String(setUrlErr)}`,
                );
                log = log;
                await tick();
              }
            } catch (installErr: any) {
              log.push(
                `Error installing archive: ${installErr.message || String(installErr)}`,
              );
              log = log;
              await tick();
              error = installErr.message || String(installErr);
              hadError = true;
            }
          } catch (modErr: any) {
            log.push(
              `Error downloading mod: ${modErr.message || String(modErr)}`,
            );
            log = log;
            await tick();
            error = modErr.message || String(modErr);
            hadError = true;
          }
        }
      }
      // Always refresh mod list after processing
      try {
        await getInstalledModGroups();
        log.push("Refreshed mod list.");
        log = log;
        await tick();
      } catch (refreshErr: any) {
        log.push(
          `Error refreshing mod list: ${refreshErr.message || String(refreshErr)}`,
        );
        log = log;
        await tick();
      }
      if (!hadError) {
        log.push("All mods processed.");
        log = log;
        await tick();
        // Do not auto-close modal; let user close it manually
      } else {
        log.push("Some mods failed. Please review the log above.");
        log = log;
        await tick();
      }
    } catch (e: any) {
      log.push(`Unexpected error: ${e.message || String(e)}`);
      await tick();
      error = e.message || String(e);
    } finally {
      isLoading = false;
    }
  }
</script>

{#if isVisible}
  <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
    <div
      class="bg-white dark:bg-zinc-900 rounded-lg shadow-2xl w-[480px] p-6 border border-gray-300 dark:border-zinc-700"
    >
      <h2 class="text-xl font-bold mb-4">
        {mode === "update" ? "Updating Modpack" : "Add Modpack"}
      </h2>
      {#if existingUrl && mode === "add"}
        <div class="mb-4 text-sm text-gray-700 dark:text-gray-300">
          A modpack URL is already configured. Adding a new modpack will append
          mods to the existing installation.
        </div>
      {/if}
      {#if mode === "add"}
        <label for="modpack-url" class="block mb-2 text-sm font-medium"
          >Modpack URL</label
        >
        <input
          id="modpack-url"
          class="input w-full mb-2"
          type="text"
          bind:value={url}
          placeholder="https://.../modpack.json"
          disabled={isLoading}
        />
        <button
          class="btn btn-sm btn-primary mb-4"
          on:click={handleSave}
          disabled={isLoading || !url}
        >
          {isLoading ? "Validating..." : "Save"}
        </button>
      {:else}
        <div class="mb-4 text-sm text-gray-700 dark:text-gray-300">
          The modpack will be updated from the configured URL.<br />
          {#if currentVersion && newVersion}
            <div class="mt-2">
              <span class="font-semibold">Current version:</span>
              <span class="font-mono">{currentVersion}</span><br />
              <span class="font-semibold">New version:</span>
              <span class="font-mono">{newVersion}</span>
            </div>
          {/if}
        </div>
        <button
          class="btn btn-sm btn-primary mb-4"
          on:click={handleSave}
          disabled={isLoading}
        >
          {isLoading ? "Updating..." : "Start Update"}
        </button>
      {/if}
      {#if error}
        <div class="text-red-500 text-sm mb-2">{error}</div>
      {/if}
      <div
        bind:this={logDiv}
        class="bg-zinc-100 dark:bg-zinc-800 rounded p-2 text-xs h-32 overflow-y-auto mb-4"
      >
        {#each log as line}
          <div>{line}</div>
        {/each}
      </div>
      <div class="flex justify-end gap-2">
        <button class="btn btn-sm" on:click={copyLog}>Copy Log</button>
        <button class="btn btn-sm" on:click={saveLog}>Save Log</button>
        <button class="btn btn-sm" on:click={close} disabled={isLoading}
          >Close</button
        >
      </div>
    </div>
  </div>
{/if}
