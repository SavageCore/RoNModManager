<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import { onMount, afterUpdate } from "svelte";
  const dispatch = createEventDispatcher();

  export let isVisible = false;

  let url = "";
  let log: string[] = [];
  let isLoading = false;
  let isValid = false;
  let error = "";
  let logDiv: HTMLDivElement | null;

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
    url = "";
    log = [];
    isLoading = false;
    isValid = false;
    error = "";
    dispatch("close");
  }

  import {
    addModIoMod,
    fetchModpackJson,
    downloadModArchive,
    installLocalMod,
    getInstalledModGroups,
    getConfig,
    modioSubscribe,
    updateModSourceUrl,
    readManifestForArchive,
    fetchModioRemoteInfo,
    getModioSubscriptionStatus,
    fileExists,
    getArchiveRootPath,
  } from "$lib/api/commands";
  import { tick } from "svelte";
  import { operationStatusStore } from "$lib/stores/operationStatus";

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
      let parsed;
      try {
        data = await fetchModpackJson(url);
      } catch (fetchErr) {
        log.push(
          `Failed to fetch modpack: ${fetchErr.message || String(fetchErr)}`,
        );
        await tick();
        error = `Failed to fetch modpack: ${fetchErr.message || String(fetchErr)}`;
        isLoading = false;
        return;
      }
      const modEntries = Object.entries(data.mods);
      log.push("Checking mods folder...");
      await tick();
      if (
        modEntries.length === 0 &&
        (!data.subscriptions || Object.keys(data.subscriptions).length === 0)
      ) {
        log.push("No mods or subscriptions found in modpack.");
        error = "No mods or subscriptions found in modpack.";
        isLoading = false;
        return;
      }
      log.push("Mods folder found.");
      await tick();
      // Get base URL for self-hosted downloads
      const baseUrl = url.replace(/\/[^/]*$/, "");
      // Get OAuth token once for all mod.io mods
      let oauthToken: string | null = null;
      let modioApiKey: string | null = null;
      let modioGameId: string | null = null;
      try {
        const config = await getConfig();
        oauthToken = config.oauth_token;
        modioApiKey = config.modio_api_key;
        modioGameId = config.modio_game_id
          ? String(config.modio_game_id)
          : null;
      } catch (e) {
        log.push("Warning: Could not retrieve mod.io credentials from config.");
        await tick();
      }

      // 1. Process all mods in the mods object
      for (const [modFile, modInfo] of modEntries) {
        const src = modInfo.source_url || "";
        log.push(`Processing self-hosted mod: ${modFile} ...`);
        log = log;
        // Download from self-hosted server or Nexus
        // Manifest hash check logic
        let manifestHashMatched = false;
        try {
          let manifest = null;
          try {
            manifest = await readManifestForArchive(modFile);
          } catch (err) {
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
              } catch (installErr) {
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
              } catch (setUrlErr) {
                log.push(
                  `Warning: Failed to set source_url: ${setUrlErr.message || String(setUrlErr)}`,
                );
                log = log;
                await tick();
              }
            } catch (installErr) {
              log.push(
                `Error installing archive: ${installErr.message || String(installErr)}`,
              );
              log = log;
              await tick();
              error = installErr.message || String(installErr);
              hadError = true;
            }
          } catch (modErr) {
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
      // 2. Process all mod.io subscriptions
      if (data.subscriptions && typeof data.subscriptions === "object") {
        const processedModIoUrls = new Set();
        for (const [subUrl, enabled] of Object.entries(data.subscriptions)) {
          if (!enabled) continue;
          if (!subUrl.includes("mod.io")) continue;
          if (processedModIoUrls.has(subUrl)) continue;
          log.push(`Subscribing to mod '${subUrl}' on mod.io...`);
          log = log;
          await tick();
          operationStatusStore.setTemporaryMessage(
            `Subscribing to mod '${subUrl}'...`,
          );
          try {
            const match = subUrl.match(/\/m\/([^/]+)/);
            if (!match) {
              throw new Error(
                "Could not extract mod slug from mod.io URL: " + subUrl,
              );
            }
            const modSlug = match[1];
            // Resolve mod slug to numeric mod ID
            let modId = null;
            if (!modioApiKey || !modioGameId) {
              throw new Error("mod.io API key or game ID not set in config.");
            }
            try {
              const resp = await fetch(
                `https://api.mod.io/v1/games/${modioGameId}/mods?name_id=${modSlug}&api_key=${modioApiKey}`,
              );
              const data = await resp.json();
              if (data && data.data && data.data[0] && data.data[0].id) {
                modId = data.data[0].id;
              } else {
                throw new Error(
                  "Could not resolve mod slug to numeric ID: " + modSlug,
                );
              }
            } catch (e) {
              throw new Error(
                "Failed to resolve mod slug to numeric ID: " +
                  modSlug +
                  ", " +
                  (e.message || e),
              );
            }
            if (!oauthToken) {
              throw new Error(
                "No OAuth token available for mod.io subscription.",
              );
            }
            const subscriptionStatus = await getModioSubscriptionStatus({
              mod_id: String(modId),
              oauth_token: oauthToken,
            });

            if (subscriptionStatus === "subscribed") {
              log.push(
                `Already subscribed to mod '${modSlug}' (ID ${modId}). Skipping subscription.`,
              );
              log = log;
              await tick();
            } else {
              await modioSubscribe({
                mod_id: String(modId),
                oauth_token: oauthToken,
              });
              log.push(`Subscribed to mod.io mod '${modSlug}' (ID ${modId}).`);
              log = log;
              await tick();
            }

            // Fetch remote md5 and archive name from backend
            const remoteInfo = await fetchModioRemoteInfo(subUrl);
            let manifest = null;
            try {
              manifest = await readManifestForArchive(remoteInfo.archive_name);
            } catch (err) {
              log.push(
                `Could not read manifest for ${remoteInfo.archive_name} (backend error: ${err && err.message ? err.message : String(err)})`,
              );
              log = log;
              await tick();
            }
            if (
              remoteInfo.remote_md5 &&
              manifest &&
              manifest.content_hash &&
              remoteInfo.remote_md5 === manifest.content_hash
            ) {
              log.push(
                "Remote mod.io file hash matches manifest and modpack. Skipping download.",
              );
              log = log;
              await tick();

              const fileExistsResult = await fileExists(
                `/home/savagecore/.local/share/ronmodmanager-dev/staged/archives/${remoteInfo.archive_name}`,
              );
              if (fileExistsResult) {
                continue;
              } else {
                log.push(
                  `Hash matches modpack but archive not found at expected path: /home/savagecore/.local/share/ronmodmanager-dev/staged/archives/${remoteInfo.archive_name}`,
                );
                log = log;
                await tick();
              }
            }
            const result = await addModIoMod(subUrl);
            log.push(`Downloading '${result.name}' from mod.io...`);
            log = log;
            await tick();
            await new Promise((resolve) => setTimeout(resolve, 500));
            log.push(`Downloaded '${result.archiveName}'. Extracting...`);
            log = log;
            await tick();
            await new Promise((resolve) => setTimeout(resolve, 500));
            try {
              await updateModSourceUrl(result.archiveName, subUrl);
              // log.push(`Set source_url for '${result.archiveName}'.`);
              // log = log;
              // await tick();
            } catch (setUrlErr) {
              log.push(
                `Warning: Failed to set source_url: ${setUrlErr.message || String(setUrlErr)}`,
              );
              log = log;
              await tick();
            }
            log.push(`Finished installing '${result.name}'.`);
            log = log;
            await tick();
          } catch (modErr) {
            log.push(
              `Error installing mod: ${modErr.message || String(modErr)}`,
            );
            log = log;
            await tick();
            error = modErr.message || String(modErr);
            hadError = true;
          }
        }
      } // <-- close subscriptions for-loop

      // Always refresh mod list after processing
      try {
        await getInstalledModGroups();
        log.push("Refreshed mod list.");
        log = log;
        await tick();
      } catch (refreshErr) {
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
    } catch (e) {
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
      <h2 class="text-xl font-bold mb-4">Add Modpack</h2>
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

<style>
  /* All Tailwind classes should be used directly in class attributes. Remove @apply usage. */
</style>
