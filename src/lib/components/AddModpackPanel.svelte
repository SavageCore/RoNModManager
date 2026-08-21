<script lang="ts">
  import { addModpackPanelStore } from "$lib/stores/addModpackPanelStore";
  import type { ModInfo } from "$lib/types/modpack";

  import { get } from "svelte/store";
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
        if ($addModpackPanelStore.mode === "add") {
          // Prefer url from store (deeplink), fallback to config
          url = get(addModpackPanelStore).url || existingUrl || "";
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

    const pushLog = async (line: string) => {
      log = [...log, line];
      await tick();
    };

    // Serialises concurrent callers so modal-based interaction (Nexus file
    // selection / manual download waits) never overlaps itself.
    let interactionChain: Promise<void> = Promise.resolve();
    const withInteractionLock = <T,>(fn: () => Promise<T>): Promise<T> => {
      const run = interactionChain.then(() => fn());
      interactionChain = run.then(
        () => undefined,
        () => undefined,
      );
      return run;
    };

    // Bounded-concurrency worker: keeps at most `limit` fns running at once.
    async function runBounded<T>(
      items: T[],
      limit: number,
      work: (item: T) => Promise<void>,
    ) {
      let idx = 0;
      const runners = Array.from(
        { length: Math.min(limit, items.length) },
        () =>
          (async () => {
            while (true) {
              const i = idx++;
              if (i >= items.length) return;
              await work(items[i]);
            }
          })(),
      );
      await Promise.all(runners);
    }

    // Install queue: downloads enqueue units here; a single worker installs them
    // in the background so installs overlap with slower downloads still running.
    type InstallUnit = {
      archivePath: string;
      archiveName: string;
      contentHash?: string | null;
      selectedPaks?: string[];
      sourceUrl?: string | null;
      fileId?: number | null;
      logLabel: string;
    };
    const installUnits: InstallUnit[] = [];
    const installWake: (() => void)[] = [];
    let installClosed = false;
    const enqueueInstall = (u: InstallUnit) => {
      installUnits.push(u);
      installWake.shift()?.();
    };
    const installWorker = (async () => {
      while (true) {
        while (installUnits.length === 0) {
          if (installClosed) return;
          await new Promise<void>((r) => installWake.push(r));
        }
        const u = installUnits.shift()!;
        try {
          await installLocalMod(
            u.archivePath,
            u.selectedPaks,
            u.contentHash ?? undefined,
          );
          await updateModSourceUrl(u.archiveName, u.sourceUrl ?? "").catch(
            () => {},
          );
          if (u.fileId != null) {
            await updateNexusFileId(u.archiveName, u.fileId).catch(() => {});
          }
          addModpackPanelStore.notifyModInstalled();
          await pushLog(`Installed '${u.logLabel}'.`);
        } catch (installErr: any) {
          await pushLog(
            `Error installing '${u.logLabel}': ${installErr.message || String(installErr)}`,
          );
          error = installErr.message || String(installErr);
          hadError = true;
        }
      }
    })();

    try {
      let modpackUrl: string | null = url;
      if ($addModpackPanelStore.mode === "update") {
        const config = await getConfig();
        modpackUrl = config.modpack_url;
        url = modpackUrl || "";
      }

      if (modpackUrl) {
        await pushLog(`Fetching modpack from URL: ${modpackUrl} ...`);
        try {
          data = await fetchModpackJson(modpackUrl);
        } catch (fetchErr: any) {
          await pushLog(
            `Failed to fetch modpack: ${fetchErr.message || String(fetchErr)}`,
          );
          error = `Failed to fetch modpack: ${fetchErr.message || String(fetchErr)}`;
          isLoading = false;
          return;
        }
      }

      try {
        await updateConfig({
          modpack_url: url,
          modpack_version: data.version,
        });
        await pushLog("Saved modpack URL and version to config.");
      } catch {
        await pushLog(
          "Warning: Could not update config with modpack URL/version.",
        );
      }

      const modEntries = Object.entries(data.mods);
      await pushLog("Checking mods folder...");
      if (modEntries.length === 0) {
        await pushLog("No mods found in modpack.");
        error = "No mods found in modpack.";
        isLoading = false;
        return;
      }
      await pushLog("Mods folder found.");
      await pushLog("---");
      const baseUrl = url.replace(/\/[^/]*$/, "");
      const isNexusPremium = await checkNexusPremium();

      let modCount = 0;
      const tasks: Array<() => Promise<void>> = [];

      for (const [modFile, modInfo] of modEntries as [string, ModInfo][]) {
        modCount++;
        const src = modInfo.source_url || "";

        if (src.toLowerCase().includes("mod.io")) {
          tasks.push(async () => {
            await pushLog(`Installing mod.io mod: ${modFile}...`);
            try {
              const remoteInfo = await fetchModioRemoteInfo(src);
              let manifest = null;
              try {
                manifest = await readManifestForArchive(
                  remoteInfo.archive_name,
                );
              } catch {}
              const archivePath = `${archiveRootPath}/${remoteInfo.archive_name}`;
              if (
                remoteInfo.remote_md5 &&
                manifest?.content_hash === remoteInfo.remote_md5 &&
                (await fileExists(archivePath))
              ) {
                await pushLog(`Already up-to-date, skipping download.`);
                if (!manifest?.source_url && src) {
                  await updateModSourceUrl(remoteInfo.archive_name, src).catch(
                    () => {},
                  );
                }
                return;
              }
              const result = await addModIoMod(src);
              enqueueInstall({
                archivePath: result.archivePath,
                archiveName: result.archiveName,
                contentHash: result.contentHash,
                selectedPaks: modInfo.selected_pak_files ?? undefined,
                sourceUrl: result.sourceUrl,
                logLabel: result.name,
              });
            } catch (modErr: any) {
              await pushLog(
                `Error installing mod.io mod: ${modErr.message || String(modErr)}`,
              );
              error = modErr.message || String(modErr);
              hadError = true;
            }
          });
          continue;
        }

        if (isNexusUrl(src)) {
          tasks.push(async () => {
            await pushLog(`Checking Nexus mod: ${modFile}...`);
            let manifest = null;
            try {
              manifest = await readManifestForArchive(modFile);
            } catch {}
            const archivePath = `${archiveRootPath}/${modFile}`;
            const isUpToDate =
              manifest?.content_hash &&
              modInfo.content_hash &&
              manifest.content_hash === modInfo.content_hash &&
              (await fileExists(archivePath).catch(() => false));
            if (isUpToDate) {
              await pushLog(`Already up-to-date, skipping download.`);
              return;
            }

            // Resolve target Nexus file IDs. Only the quick file-picker modal is
            // serialised here - never the downloads themselves.
            let chosenFileIds: number[] | null =
              modInfo.nexus_file_id != null ? [modInfo.nexus_file_id] : null;
            try {
              if (chosenFileIds == null) {
                chosenFileIds = await withInteractionLock(async () => {
                  const fileOptions = await listNexusFileOptions(src);
                  if (fileOptions.length > 1) {
                    const chosen = await requestNexusFileSelection(
                      modFile,
                      fileOptions,
                    );
                    if (chosen === null) {
                      await pushLog(`Skipped: ${modFile} (cancelled)`);
                      return null as unknown as number[];
                    }
                    return chosen.map((f) => f.fileId);
                  }
                  if (fileOptions.length === 1) return [fileOptions[0].fileId];
                  return [];
                });
              }
            } catch (optErr: any) {
              await pushLog(
                `Error listing Nexus files: ${optErr.message || String(optErr)}`,
              );
              error = optErr.message || String(optErr);
              hadError = true;
              return;
            }
            if (chosenFileIds == null) return;

            const fileIds: (number | undefined)[] =
              chosenFileIds.length > 0 ? chosenFileIds : [undefined];

            if (isNexusPremium) {
              for (const fid of fileIds) {
                await pushLog(`Downloading Nexus mod: ${modFile}...`);
                try {
                  const result = await addNexusMod(src, fid);
                  enqueueInstall({
                    archivePath: result.archivePath,
                    archiveName: result.archiveName,
                    contentHash: result.contentHash,
                    selectedPaks: modInfo.selected_pak_files ?? undefined,
                    sourceUrl: result.sourceUrl,
                    fileId: result.fileId,
                    logLabel: result.name,
                  });
                } catch (modErr: any) {
                  await pushLog(
                    `Error downloading Nexus mod ${modFile}: ${modErr.message || String(modErr)}`,
                  );
                  error = modErr.message || String(modErr);
                  hadError = true;
                }
              }
              return;
            }

            // Non-premium: try the self-hosted server first, fall back to a manual
            // Nexus download. Manual watchers run concurrently like any other
            // download - each polls ~/Downloads independently and its install
            // starts the moment its file lands, while other downloads continue.
            let selfHosted = false;
            let downloadedHash: string | null = null;
            try {
              const downloadUrl = `${baseUrl}/mods/${encodeURIComponent(modFile)}`;
              const result = await downloadModArchive(
                downloadUrl,
                modFile,
                modInfo.content_hash,
              );
              downloadedHash = result.contentHash;
              selfHosted = true;
              await pushLog(
                result.reusedLocal
                  ? `Used local copy of '${modFile}' from Downloads.`
                  : `Downloaded '${modFile}' from server.`,
              );
              enqueueInstall({
                archivePath,
                archiveName: modFile,
                contentHash: downloadedHash,
                selectedPaks: modInfo.selected_pak_files ?? undefined,
                sourceUrl: src,
                logLabel: modFile,
              });
            } catch {
              await pushLog(
                `Server download failed, falling back to Nexus manual download...`,
              );
            }

            if (!selfHosted) {
              const unlistenWaiting = await listen<{
                prettyName: string | null;
                fileName: string;
                modUrl: string;
              }>("nexus_free_download_waiting", (event) => {
                void pushLog(
                  `Waiting for manual download: ${event.payload.fileName}...`,
                );
              });
              try {
                for (const fid of fileIds) {
                  try {
                    await pushLog(
                      `Manual download required: ${modFile}. Download it in the browser - installing as soon as it lands...`,
                    );
                    const result = await addNexusMod(src, fid);
                    enqueueInstall({
                      archivePath: result.archivePath,
                      archiveName: result.archiveName,
                      contentHash: result.contentHash,
                      selectedPaks: modInfo.selected_pak_files ?? undefined,
                      sourceUrl: result.sourceUrl,
                      fileId: result.fileId,
                      logLabel: result.name,
                    });
                  } catch (modErr: any) {
                    await pushLog(
                      `Error downloading Nexus mod ${modFile}: ${modErr.message || String(modErr)}`,
                    );
                    error = modErr.message || String(modErr);
                    hadError = true;
                  }
                }
              } finally {
                unlistenWaiting();
              }
            }
          });
          continue;
        }

        // Generic self-hosted mod.
        tasks.push(async () => {
          await pushLog(`Processing mod: ${modFile} ...`);
          let manifestHashMatched = false;
          try {
            let manifest = null;
            try {
              manifest = await readManifestForArchive(modFile);
            } catch (err: any) {
              await pushLog(
                `Could not read manifest for ${modFile} (backend error: ${err && err.message ? err.message : String(err)})`,
              );
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
                await pushLog(
                  `Error checking file existence for ${archivePath}. Will attempt download.`,
                );
              }
              if (fileExistsResult) {
                await pushLog(`Already installed and up-to-date, skipping.`);
                if (!manifest?.source_url && src) {
                  await updateModSourceUrl(modFile, src).catch(() => {});
                }
                manifestHashMatched = true;
              } else {
                await pushLog(
                  `Hash matches modpack but archive not found at expected path: ${archivePath}`,
                );
              }
            } else {
              await pushLog(
                `File does not exist or hash mismatch (manifest hash: ${manifest && manifest.content_hash ? manifest.content_hash : "N/A"}, modpack hash: ${modInfo.content_hash ? modInfo.content_hash : "N/A"})`,
              );
            }
          } catch {}
          if (!manifestHashMatched) {
            const downloadUrl = `${baseUrl}/mods/${encodeURIComponent(modFile)}`;
            await pushLog(`Downloading...`);
            try {
              const result = await downloadModArchive(
                downloadUrl,
                modFile,
                modInfo.content_hash,
              );
              const downloadedHash = result.contentHash;
              await pushLog(
                result.reusedLocal
                  ? `Used local copy of '${modFile}' from Downloads.`
                  : `Downloaded '${modFile}'.`,
              );
              enqueueInstall({
                archivePath: `${archiveRootPath}/${modFile}`,
                archiveName: modFile,
                contentHash: downloadedHash,
                selectedPaks: modInfo.selected_pak_files ?? undefined,
                sourceUrl: src,
                logLabel: modFile,
              });
            } catch (modErr: any) {
              await pushLog(
                `Error downloading mod: ${modErr.message || String(modErr)}`,
              );
              error = modErr.message || String(modErr);
              hadError = true;
            }
          }
        });
      }

      // Run all download tasks with limited concurrency. Installs are drained
      // by the background worker above as each download completes, so a slow
      // Nexus file no longer blocks already-downloaded mods from installing.
      await runBounded(tasks, 4, async (task) => {
        await task();
      });

      installClosed = true;
      installWake.shift()?.();
      await installWorker;

      let brokenCount = 0;
      let metaErr: any = null;
      try {
        await applyModpackProfileMetadata(data);
        brokenCount = data.broken ? Object.keys(data.broken).length : 0;
      } catch (err: any) {
        metaErr = err;
      }

      if (modCount > 0) {
        await pushLog("---");
      }
      if (brokenCount > 0) {
        await pushLog(`Applied ${brokenCount} broken note(s) from modpack.`);
      }
      if (metaErr) {
        await pushLog(
          `Warning: Could not apply modpack metadata - ${metaErr?.message ?? String(metaErr)}`,
        );
      }
      if (!hadError) {
        await pushLog("All mods processed.");
      } else {
        await pushLog("Some mods failed. Please review the log above.");
      }
      addModpackPanelStore.notifyDone();
    } catch (e: any) {
      await pushLog(`Unexpected error: ${e.message || String(e)}`);
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
