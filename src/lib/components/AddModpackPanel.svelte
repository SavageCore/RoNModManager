<script lang="ts">
  import { addModpackPanelStore } from "$lib/stores/addModpackPanelStore";
  import type { ModInfo } from "$lib/types/modpack";

  let url = "";
  let log: string[] = [];
  let isLoading = false;
  let error = "";
  let existingUrl: string | null = null;

  $: addModpackPanelStore.setActivity(isLoading || log.length > 0);

  function close() {
    addModpackPanelStore.close();
  }

  import {
    addModIoMod,
    addNexusMod,
    applyModpackProfileMetadata,
    checkNexusPremium,
    downloadModArchive,
    fetchModpackJson,
    fetchModioRemoteInfo,
    fileExists,
    getArchiveRootPath,
    getConfig,
    installLocalMod,
    listNexusFileOptions,
    readManifestForArchive,
    updateConfig,
    updateModSourceUrl,
    updateNexusFileId,
  } from "$lib/api/commands";
  import { requestNexusFileSelection } from "$lib/stores/nexusFileSelection";
  import { listen } from "@tauri-apps/api/event";
  import { tick } from "svelte";
  import LogPanel from "./LogPanel.svelte";

  function isNexusUrl(value: string): boolean {
    return value.includes("nexusmods.com/") && value.includes("/mods/");
  }

  $: if ($addModpackPanelStore.isOpen) {
    (async () => {
      try {
        const config = await getConfig();
        existingUrl = config.modpack_url || null;
        if (existingUrl && $addModpackPanelStore.mode === "add") {
          url = existingUrl;
        }
      } catch {
        // ignore
      }
    })();
  }

  async function handleSave() {
    log = ["Validating URL..."];
    error = "";
    isLoading = true;
    let hadError = false;
    let data;
    const archiveRootPath = await getArchiveRootPath();
    try {
      let modpackUrl: string | null = url;
      if ($addModpackPanelStore.mode === "update") {
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
      const isNexusPremium = await checkNexusPremium();

      type PendingNexus = {
        modFile: string;
        modInfo: ModInfo;
        promise: Promise<Awaited<ReturnType<typeof addNexusMod>>>;
      };
      const pendingNexus: PendingNexus[] = [];

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
              if (!manifest?.source_url && src) {
                await updateModSourceUrl(remoteInfo.archive_name, src).catch(
                  () => {},
                );
              }
            } else {
              const result = await addModIoMod(src);
              await installLocalMod(
                result.archivePath,
                modInfo.selected_pak_files ?? undefined,
                result.contentHash,
              );
              await updateModSourceUrl(
                result.archiveName,
                result.sourceUrl,
              ).catch(() => {});
              addModpackPanelStore.notifyModInstalled();
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

        if (isNexusUrl(src) && isNexusPremium) {
          log.push(`Checking Nexus mod: ${modFile}...`);
          log = log;
          await tick();
          try {
            let manifest = null;
            try {
              manifest = await readManifestForArchive(modFile);
            } catch {}
            const archivePath = `${archiveRootPath}/${modFile}`;
            if (
              manifest?.content_hash &&
              modInfo.content_hash &&
              manifest.content_hash === modInfo.content_hash &&
              (await fileExists(archivePath).catch(() => false))
            ) {
              log.push(`Already up-to-date, skipping download.`);
              log = log;
              await tick();
              continue;
            }

            let chosenFileId: number | undefined =
              modInfo.nexus_file_id ?? undefined;
            if (chosenFileId == null) {
              const fileOptions = await listNexusFileOptions(src);
              if (fileOptions.length > 1) {
                const chosen = await requestNexusFileSelection(
                  modFile,
                  fileOptions,
                );
                if (chosen === null) {
                  log.push(`Skipped: ${modFile} (cancelled)`);
                  log = log;
                  await tick();
                  continue;
                }
                chosenFileId = chosen.fileId;
              } else if (fileOptions.length === 1) {
                chosenFileId = fileOptions[0].fileId;
              }
            }

            log.push(`Nexus download queued, continuing with other mods...`);
            log = log;
            await tick();
            pendingNexus.push({
              modFile,
              modInfo,
              promise: addNexusMod(src, chosenFileId),
            });
          } catch (modErr: any) {
            log.push(
              `Error queuing Nexus mod: ${modErr.message || String(modErr)}`,
            );
            log = log;
            await tick();
            error = modErr.message || String(modErr);
            hadError = true;
          }
          continue;
        }

        if (isNexusUrl(src)) {
          // Non-premium: try self-hosted first, fall back to Nexus manual download
          log.push(`Checking Nexus mod (non-premium): ${modFile}...`);
          log = log;
          await tick();
          const archivePath = `${archiveRootPath}/${modFile}`;
          let manifestHashMatched = false;
          try {
            const manifest = await readManifestForArchive(modFile).catch(
              () => null,
            );
            if (
              manifest?.content_hash &&
              modInfo.content_hash &&
              manifest.content_hash === modInfo.content_hash &&
              (await fileExists(archivePath).catch(() => false))
            ) {
              log.push(`Already up-to-date, skipping.`);
              log = log;
              await tick();
              manifestHashMatched = true;
            }
          } catch {}
          if (!manifestHashMatched) {
            const downloadUrl = `${baseUrl}/mods/${encodeURIComponent(modFile)}`;
            let selfHosted = false;
            let downloadedHash: string | null = null;
            try {
              const result = await downloadModArchive(
                downloadUrl,
                modFile,
                modInfo.content_hash,
              );
              downloadedHash = result.contentHash;
              selfHosted = true;
              log.push(
                result.reusedLocal
                  ? `Used local copy of '${modFile}' from Downloads.`
                  : `Downloaded '${modFile}' from server.`,
              );
              log = log;
              await tick();
            } catch {
              log.push(
                `Server download failed, falling back to Nexus manual download...`,
              );
              log = log;
              await tick();
            }
            if (selfHosted) {
              try {
                await installLocalMod(
                  archivePath,
                  modInfo.selected_pak_files ?? undefined,
                  downloadedHash ?? undefined,
                );
                await updateModSourceUrl(modFile, src).catch(() => {});
                addModpackPanelStore.notifyModInstalled();
                log.push(`Installed '${modFile}'.`);
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
            } else {
              let chosenFileId: number | undefined =
                modInfo.nexus_file_id ?? undefined;
              if (chosenFileId == null) {
                try {
                  const fileOptions = await listNexusFileOptions(src);
                  if (fileOptions.length > 1) {
                    const chosen = await requestNexusFileSelection(
                      modFile,
                      fileOptions,
                    );
                    if (chosen === null) {
                      log.push(`Skipped: ${modFile} (cancelled)`);
                      log = log;
                      await tick();
                      continue;
                    }
                    chosenFileId = chosen.fileId;
                  } else if (fileOptions.length === 1) {
                    chosenFileId = fileOptions[0].fileId;
                  }
                } catch (optErr: any) {
                  log.push(
                    `Error listing Nexus files: ${optErr.message || String(optErr)}`,
                  );
                  log = log;
                  await tick();
                  error = optErr.message || String(optErr);
                  hadError = true;
                  continue;
                }
              }
              log.push(`Nexus download queued, continuing with other mods...`);
              log = log;
              await tick();
              pendingNexus.push({
                modFile,
                modInfo,
                promise: addNexusMod(src, chosenFileId),
              });
            }
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
              if (!manifest?.source_url && src) {
                await updateModSourceUrl(modFile, src).catch(() => {});
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
          try {
            const result = await downloadModArchive(
              downloadUrl,
              modFile,
              modInfo.content_hash,
            );
            const downloadedHash = result.contentHash;
            log.push(
              result.reusedLocal
                ? `Used local copy of '${modFile}' from Downloads.`
                : `Downloaded '${modFile}'.`,
            );
            log = log;
            await tick();
            const archivePath = `${archiveRootPath}/${modFile}`;
            try {
              await installLocalMod(
                archivePath,
                modInfo.selected_pak_files ?? undefined,
                downloadedHash ?? undefined,
              );
              try {
                await updateModSourceUrl(modFile, src);
              } catch (setUrlErr: any) {
                log.push(
                  `Warning: Failed to set source_url: ${setUrlErr.message || String(setUrlErr)}`,
                );
                log = log;
                await tick();
              }
              addModpackPanelStore.notifyModInstalled();
              log.push(`Installed '${modFile}'.`);
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
      if (pendingNexus.length > 0) {
        log.push("---");
        log.push("Processing Nexus downloads...");
        log = log;
        await tick();
        const unlistenWaiting = await listen<{
          prettyName: string | null;
          fileName: string;
          modUrl: string;
        }>("nexus_free_download_waiting", (event) => {
          log.push(`Waiting for manual download: ${event.payload.fileName}...`);
          log = log;
        });
        try {
          for (const pending of pendingNexus) {
            log.push(`Installing Nexus mod: ${pending.modFile}...`);
            log = log;
            await tick();
            try {
              const result = await pending.promise;
              await installLocalMod(
                result.archivePath,
                pending.modInfo.selected_pak_files ?? undefined,
                result.contentHash,
              );
              await updateModSourceUrl(
                result.archiveName,
                result.sourceUrl,
              ).catch(() => {});
              if (result.fileId != null) {
                await updateNexusFileId(
                  result.archiveName,
                  result.fileId,
                ).catch(() => {});
              }
              addModpackPanelStore.notifyModInstalled();
              log.push(`Installed '${result.name}' from Nexus.`);
              log = log;
              await tick();
            } catch (modErr: any) {
              log.push(
                `Error installing Nexus mod ${pending.modFile}: ${modErr.message || String(modErr)}`,
              );
              log = log;
              await tick();
              error = modErr.message || String(modErr);
              hadError = true;
            }
          }
        } finally {
          unlistenWaiting();
        }
      }
      let brokenCount = 0;
      let metaErr: any = null;
      try {
        await applyModpackProfileMetadata(data);
        brokenCount = data.broken ? Object.keys(data.broken).length : 0;
      } catch (err: any) {
        metaErr = err;
      }

      if (modCount > 0) {
        log.push("---");
        log = log;
        await tick();
      }
      if (brokenCount > 0) {
        log.push(`Applied ${brokenCount} broken note(s) from modpack.`);
        log = log;
        await tick();
      }
      if (metaErr) {
        log.push(
          `Warning: Could not apply modpack metadata - ${metaErr?.message ?? String(metaErr)}`,
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
      addModpackPanelStore.notifyDone();
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
  title={$addModpackPanelStore.mode === "update"
    ? "Update Modpack"
    : "Add Modpack"}
  isVisible={$addModpackPanelStore.isOpen}
  {isLoading}
  {log}
  width="480px"
  maxHeight="480px"
  logFilename="modpack-log"
  on:close={close}
  on:clear={() => {
    log = [];
    error = "";
    addModpackPanelStore.close();
  }}
>
  <div slot="controls" class="px-3 pt-3 pb-2 shrink-0">
    {#if existingUrl && url !== existingUrl && $addModpackPanelStore.mode === "add"}
      <p class="text-xs mb-2" style="color: var(--clr-text-secondary);">
        A modpack URL is already configured. Adding a new modpack will append
        mods to the existing installation.
      </p>
    {/if}
    {#if $addModpackPanelStore.mode === "add"}
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
          {isLoading
            ? "Working..."
            : url === existingUrl && existingUrl
              ? "Update"
              : "Start"}
        </button>
      </div>
    {:else}
      <div class="text-xs mb-2" style="color: var(--clr-text-secondary);">
        Updating from configured URL.
        {#if $addModpackPanelStore.currentVersion && $addModpackPanelStore.newVersion}
          <span class="ml-2" style="color: var(--clr-text);">
            {$addModpackPanelStore.currentVersion} → {$addModpackPanelStore.newVersion}
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
