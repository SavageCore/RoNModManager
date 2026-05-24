<script lang="ts">
  import { createEventDispatcher } from "svelte";
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
  let existingUrl: string | null = null;

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
  import { tick } from "svelte";
  import LogPanel from "./LogPanel.svelte";

  $: if (isVisible) {
    (async () => {
      try {
        const config = await getConfig();
        existingUrl = config.modpack_url || null;
      } catch {
        // ignore
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
      let modpackUrl: string | null = url;
      if (mode === "update") {
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

      try {
        await updateConfig({ modpack_url: url, modpack_version: data.version });
        log.push("Saved modpack URL and version to config.");
        await tick();
      } catch {
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
      log.push("---");
      log = log;
      await tick();
      const baseUrl = url.replace(/\/[^/]*$/, "");

      let modCount = 0;
      for (const [modFile, modInfo] of modEntries as [string, ModInfo][]) {
        if (modCount > 0) {
          log.push("---");
          log = log;
          await tick();
        }
        modCount++;
        const src = modInfo.source_url || "";

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
              await installLocalMod(
                result.archivePath,
                modInfo.selected_pak_files ?? undefined,
              );
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
            let fileExistsResult = false;
            try {
              fileExistsResult = await fileExists(archivePath);
            } catch {
              log.push(
                `Error checking file existence for ${archivePath}. Will attempt download.`,
              );
              log = log;
              await tick();
            }
            if (fileExistsResult) {
              log.push(`Already installed and up-to-date, skipping.`);
              log = log;
              await tick();
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
          try {
            await downloadModArchive(downloadUrl, modFile);
            log.push(`Downloaded '${modFile}'.`);
            log = log;
            await tick();
            const archivePath = `${archiveRootPath}/${modFile}`;
            try {
              await installLocalMod(
                archivePath,
                modInfo.selected_pak_files ?? undefined,
              );
              log.push(`Installed '${modFile}'.`);
              log = log;
              await tick();
              try {
                await updateModSourceUrl(modFile, src);
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
      if (modCount > 0) {
        log.push("---");
        log = log;
        await tick();
      }
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

<LogPanel
  title={mode === "update" ? "Update Modpack" : "Add Modpack"}
  {isVisible}
  {isLoading}
  {log}
  width="480px"
  maxHeight="480px"
  logFilename="modpack-log"
  on:close={close}
>
  <div slot="controls" class="px-3 pt-3 pb-2 shrink-0">
    {#if existingUrl && mode === "add"}
      <p class="text-xs mb-2" style="color: var(--clr-text-secondary);">
        A modpack URL is already configured. Adding a new modpack will append
        mods to the existing installation.
      </p>
    {/if}
    {#if mode === "add"}
      <div class="flex gap-2 mb-2">
        <input
          id="modpack-url"
          class="input flex-1 text-xs"
          style="height: 2rem;"
          type="text"
          bind:value={url}
          placeholder="https://.../modpack.json"
          disabled={isLoading}
        />
        <button
          class="btn btn-sm primary shrink-0"
          on:click={handleSave}
          disabled={isLoading || !url}
        >
          {isLoading ? "Working..." : "Start"}
        </button>
      </div>
    {:else}
      <div class="text-xs mb-2" style="color: var(--clr-text-secondary);">
        Updating from configured URL.
        {#if currentVersion && newVersion}
          <span class="ml-2" style="color: var(--clr-text);">
            {currentVersion} → {newVersion}
          </span>
        {/if}
      </div>
      <button
        class="btn btn-sm primary mb-2"
        on:click={handleSave}
        disabled={isLoading}
      >
        {isLoading ? "Updating..." : "Start Update"}
      </button>
    {/if}
    {#if error}
      <p class="text-xs mt-1" style="color: var(--clr-danger-300);">{error}</p>
    {/if}
  </div>

  {#each log as line}
    {#if line === "---"}
      <div
        class="my-1.5"
        style="border-top: 1px solid var(--adw-border-color);"
      ></div>
    {:else}
      <div class="leading-relaxed">{line}</div>
    {/if}
  {/each}
  {#if log.length === 0}
    <div style="color: var(--clr-text-secondary);">Waiting...</div>
  {/if}
</LogPanel>
