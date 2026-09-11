<script lang="ts">
  import {
    addModIoMod,
    addNexusMod,
    fetchModioRemoteInfo,
    fetchNexusModInfo,
    getArchivePakFiles,
    getTags,
    installLocalMod,
    listNexusFileOptions,
    replaceModArchive,
    setModTags,
    updateModDisplayName,
    updateModSourceUrl,
    updateNexusFileId,
  } from "$lib/api/commands";
  import { addModpackPanelStore } from "$lib/stores/addModpackPanelStore";
  import { alertStore } from "$lib/stores/alert";
  import { importLogStore } from "$lib/stores/importLogStore";
  import { modAddQueueStore } from "$lib/stores/modAddQueue";
  import { requestPakSelection } from "$lib/stores/pakSelection";
  import { requestNexusFileSelection } from "$lib/stores/nexusFileSelection";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open } from "@tauri-apps/plugin-dialog";
  import { createEventDispatcher, onDestroy, onMount } from "svelte";
  import ModalShell from "./ModalShell.svelte";

  export let isVisible = false;
  export let autoSubmitEntries: Array<{
    url: string;
    replacing: string | null;
    displayName?: string;
    /** Pre-selected Nexus file IDs resolved upstream (userscript). Skips the
     * in-app file-variant picker. */
    fileIds?: number[];
    /** When true, the backend will not open a duplicate browser tab for the
     * free (non-premium) download - the userscript already started it. */
    skipBrowserOpen?: boolean;
  }> = [];

  $: if (isVisible && autoSubmitEntries.length > 0) {
    void submitAutoEntries(autoSubmitEntries);
  }

  const dispatch = createEventDispatcher();

  let activeTab: "link" | "file" = "link";
  let isDraggingOver = false;
  let unlistenDragDrop: (() => void) | null = null;
  let modioInput = "";
  let nexusPreviewName = "";
  let nexusPreviewError = "";
  let nexusLookupTimer: ReturnType<typeof setTimeout> | null = null;
  let nexusLookupToken = 0;
  let isProcessingLinks = false;
  const pendingLinkQueue: Array<{
    input: string;
    queueId: string;
    replacingArchiveName?: string;
    displayName?: string;
    /** Pre-selected file IDs to download (skips in-app variant picker). */
    fileIds?: number[];
    /** When true, backend won't open a duplicate browser download tab. */
    skipBrowserOpen?: boolean;
  }> = [];

  $: activeQueueCount = $modAddQueueStore.items.filter(
    (item) => item.status === "queued" || item.status === "running",
  ).length;

  $: alertStyle =
    $alertStore.type === "success"
      ? "color: var(--clr-success-300, #4caf50); background: color-mix(in srgb, var(--clr-success-300, #4caf50) 18%, transparent);"
      : $alertStore.type === "error"
        ? "color: var(--clr-danger-300); background: color-mix(in srgb, var(--clr-danger-300) 18%, transparent);"
        : "color: var(--clr-primary-300); background: color-mix(in srgb, var(--clr-primary-300) 18%, transparent);";

  function cleanModUrl(value: string): string {
    // Strip hash fragments from URLs
    return value.replace(/#.*$/, "").trim();
  }

  function parseModInputs(input: string): string[] {
    return input
      .split(/\r?\n/)
      .map((line) => cleanModUrl(line))
      .filter((line) => line.length > 0 && !line.startsWith("#"));
  }

  function handlePaste(event: ClipboardEvent) {
    const text = event.clipboardData?.getData("text");
    if (!text) return;

    const cleaned = text
      .split(/\r?\n/)
      .map((line) => cleanModUrl(line))
      .join("\n");

    if (cleaned !== text) {
      event.preventDefault();
      const target = event.target as HTMLTextAreaElement;
      const start = target.selectionStart ?? 0;
      const end = target.selectionEnd ?? 0;
      modioInput = modioInput.slice(0, start) + cleaned + modioInput.slice(end);
      // Restore cursor position after inserted text
      const newPos = start + cleaned.length;
      requestAnimationFrame(() => {
        target.setSelectionRange(newPos, newPos);
      });
    }
  }

  function isNexusUrl(value: string): boolean {
    return value.includes("nexusmods.com/") && value.includes("/mods/");
  }

  // Look up a mod's display name from its Nexus/mod.io input. Used so the
  // import log shows the mod name instead of the URL/slug. Failures are
  // non-fatal - the log falls back to the raw input.
  async function resolveModName(input: string): Promise<string | null> {
    try {
      if (isNexusUrl(input)) {
        const info = await fetchNexusModInfo(input);
        return info.name || null;
      }
      const info = await fetchModioRemoteInfo(input);
      return info.name || null;
    } catch {
      return null;
    }
  }

  async function previewNexusName(input: string): Promise<void> {
    const lookupId = ++nexusLookupToken;
    nexusPreviewName = "";
    nexusPreviewError = "";

    if (!isNexusUrl(input)) {
      return;
    }

    try {
      const info = await fetchNexusModInfo(input);
      if (lookupId !== nexusLookupToken) {
        return;
      }
      nexusPreviewName = info.name;
    } catch (error) {
      if (lookupId !== nexusLookupToken) {
        return;
      }
      nexusPreviewError = String(error);
    }
  }

  $: {
    if (activeTab !== "link") {
      nexusPreviewName = "";
      nexusPreviewError = "";
    } else {
      const entries = parseModInputs(modioInput);
      const singleEntry = entries.length === 1 ? entries[0] : "";

      if (nexusLookupTimer) {
        clearTimeout(nexusLookupTimer);
        nexusLookupTimer = null;
      }

      if (!singleEntry || !isNexusUrl(singleEntry)) {
        nexusPreviewName = "";
        nexusPreviewError = "";
      } else {
        nexusLookupTimer = setTimeout(() => {
          void previewNexusName(singleEntry);
        }, 300);
      }
    }
  }

  async function handleAddViaLink(replacingArchiveName?: string) {
    const input = modioInput.trim();
    if (!input) {
      alertStore.error("Enter mod links");
      return;
    }

    const modInputs = parseModInputs(input);

    if (modInputs.length === 0) {
      alertStore.error("No valid mod inputs found");
      return;
    }

    alertStore.clear();
    modioInput = "";

    // Enqueue all submitted mods - totalQueued accumulates correctly across submissions
    for (const modInput of modInputs) {
      pendingLinkQueue.push({
        input: modInput,
        queueId: modAddQueueStore.enqueue(
          modInput,
          nexusPreviewName || undefined,
        ),
        replacingArchiveName,
      });
    }

    // Close immediately - progress is visible in the bottom bar
    closeModal();

    wakeWorker();
    void processQueue();
  }

  // Bulk auto-submit (e.g. "Update All") - each entry replaces its own archive.
  async function submitAutoEntries(
    entries: Array<{
      url: string;
      replacing: string | null;
      displayName?: string;
      fileIds?: number[];
      skipBrowserOpen?: boolean;
    }>,
  ) {
    for (const entry of entries) {
      pendingLinkQueue.push({
        input: entry.url,
        queueId: modAddQueueStore.enqueue(entry.url, entry.displayName),
        replacingArchiveName: entry.replacing ?? undefined,
        displayName: entry.displayName,
        fileIds: entry.fileIds,
        skipBrowserOpen: entry.skipBrowserOpen ?? false,
      });
    }
    closeModal();
    wakeWorker();
    void processQueue();
  }

  // Resolves current wait so newly queued items are picked up immediately.
  // Only one waiter is outstanding at a time (every wait is awaited before
  // looping), so a single resolver slot is enough. Each waiter also has a
  // timeout fallback so a lost wake can never hang the loop forever.
  let wakeResolver: (() => void) | null = null;
  let wakeTimer: ReturnType<typeof setTimeout> | null = null;
  function waitForWake(timeoutMs = 30000): Promise<void> {
    return new Promise((resolve) => {
      // A previous waiter must have settled via the other race branch -
      // disarm it so no stale timer/resolver lingers.
      if (wakeTimer) {
        clearTimeout(wakeTimer);
        wakeTimer = null;
      }
      wakeTimer = setTimeout(() => {
        wakeTimer = null;
        if (wakeResolver === wrapped) wakeResolver = null;
        resolve();
      }, timeoutMs);
      const wrapped = () => {
        if (wakeTimer) {
          clearTimeout(wakeTimer);
          wakeTimer = null;
        }
        resolve();
      };
      wakeResolver = wrapped;
    });
  }
  // Disarm the current waiter without resolving it. Returns true when a
  // waiter was still armed, meaning the race just settled via the other
  // branch (caller should yield so progress events / UI can run).
  function disarmWake(): boolean {
    if (!wakeResolver) return false;
    wakeResolver = null;
    if (wakeTimer) {
      clearTimeout(wakeTimer);
      wakeTimer = null;
    }
    return true;
  }
  function wakeWorker() {
    if (wakeResolver) {
      const resolve = wakeResolver;
      wakeResolver = null;
      if (wakeTimer) {
        clearTimeout(wakeTimer);
        wakeTimer = null;
      }
      resolve();
    }
  }

  async function processQueue() {
    // Single worker loop - if already running, the new items will be picked up naturally
    if (isProcessingLinks) return;

    isProcessingLinks = true;
    try {
      type Download = {
        promise?:
          ReturnType<typeof addNexusMod> | ReturnType<typeof addModIoMod>;
        result?:
          | Awaited<ReturnType<typeof addNexusMod>>
          | Awaited<ReturnType<typeof addModIoMod>>;
        selectedPaks?: string[];
        failed?: boolean;
        // True once the underlying download promise settles, independent of
        // whether workChain has consumed it yet. d.result alone can't be used:
        // it is only assigned inside workChain, so an already-resolved
        // download queued behind another mod's install would look "in flight"
        // and Promise.race on it would resolve instantly in a tight microtask
        // loop, starving progress events / UI.
        settled?: boolean;
      };
      type Plan = {
        entry: {
          input: string;
          queueId: string;
          replacingArchiveName?: string;
          displayName?: string;
          fileIds?: number[];
          skipBrowserOpen?: boolean;
        };
        chosenFileIds: number[];
        downloads: Download[];
        failed?: boolean;
      };
      const allPlans: Plan[] = [];

      // Serialises post-download work (PAK picker + install) so prompts never
      // stack and installs don't race each other - while the downloads
      // themselves keep running concurrently in the background.
      let workChain: Promise<void> = Promise.resolve();

      // Phase 1 serialisation: Nexus file-variant prompts must not overlap.
      let interactionChain: Promise<void> = Promise.resolve();
      const withInteractionLock = <T,>(fn: () => Promise<T>): Promise<T> => {
        const run = interactionChain.then(fn, fn);
        interactionChain = run.then(
          () => undefined,
          () => undefined,
        );
        return run;
      };

      // Single loop: process queued items, then wait for work, waking
      // immediately when new items arrive so their Phase 1 + browser tab
      // start promptly.
      let finished = false;
      while (!finished) {
        // Drain the queue, kicking off each download as we go.
        while (pendingLinkQueue.length > 0) {
          const entry = pendingLinkQueue.shift()!;
          const plan: Plan = {
            entry,
            chosenFileIds: [],
            downloads: [],
            failed: false,
          };
          allPlans.push(plan);

          // Resolve the display name up front so the import log shows the mod
          // name rather than the URL/slug while the download runs. The
          // post-download result re-applies it as a safety net.
          if (!plan.entry.displayName) {
            const resolvedName = await resolveModName(plan.entry.input);
            if (resolvedName) {
              plan.entry.displayName = resolvedName;
              modAddQueueStore.setName(plan.entry.queueId, resolvedName);
            }
          }

          // Phase 1: Ask Nexus file variant questions before downloading.
          // Serialised so prompts never overlap.
          if (isNexusUrl(plan.entry.input)) {
            try {
              // If the userscript already resolved variant(s), skip the
              // in-app picker and the browser-tab open.
              if (plan.entry.fileIds && plan.entry.fileIds.length > 0) {
                plan.chosenFileIds = [...plan.entry.fileIds];
                modAddQueueStore.markRunning(plan.entry.queueId, "Starting...");
              } else {
                const fileOptions = await withInteractionLock(async () => {
                  modAddQueueStore.markRunning(
                    plan.entry.queueId,
                    "Checking available files...",
                  );
                  return listNexusFileOptions(plan.entry.input);
                });
                let chosenFileIds: number[] = [];
                if (fileOptions.length > 1) {
                  const chosen = await withInteractionLock(async () => {
                    modAddQueueStore.markRunning(
                      plan.entry.queueId,
                      "Select file variant...",
                    );
                    importLogStore.setWaitingForInput(plan.entry.queueId);
                    const result = await requestNexusFileSelection(
                      plan.entry.displayName ||
                        nexusPreviewName ||
                        plan.entry.input,
                      fileOptions,
                    );
                    importLogStore.clearWaitingForInput(plan.entry.queueId);
                    return result;
                  });
                  if (chosen === null) {
                    modAddQueueStore.markError(plan.entry.queueId, "Cancelled");
                    plan.failed = true;
                    continue;
                  }
                  chosenFileIds = chosen.map((f) => f.fileId);
                } else if (fileOptions.length === 1) {
                  chosenFileIds = [fileOptions[0].fileId];
                }
                plan.chosenFileIds = chosenFileIds;
                modAddQueueStore.markRunning(plan.entry.queueId, "Queued");
              }
            } catch (error) {
              modAddQueueStore.markError(
                plan.entry.queueId,
                `Failed: ${String(error)}`,
              );
              plan.failed = true;
              continue;
            }
          }

          // Phase 2: Kick off download immediately (concurrent with any
          // previous downloads still in flight). Attach a settle hook so the
          // wait logic below can tell truly-pending downloads apart from
          // already-resolved ones still queued behind workChain.
          const trackSettled = (d: Download) => {
            d.promise?.then(
              () => {
                d.settled = true;
              },
              () => {
                d.settled = true;
              },
            );
          };
          if (isNexusUrl(plan.entry.input)) {
            const fileIds =
              plan.chosenFileIds.length > 0 ? plan.chosenFileIds : [undefined];
            for (const fileId of fileIds) {
              const download: Download = {
                promise: addNexusMod(
                  plan.entry.input,
                  fileId,
                  plan.entry.skipBrowserOpen,
                ),
              };
              trackSettled(download);
              plan.downloads.push(download);
            }
          } else {
            const download: Download = {
              promise: addModIoMod(plan.entry.input),
            };
            trackSettled(download);
            plan.downloads.push(download);
          }
          modAddQueueStore.markRunning(
            plan.entry.queueId,
            isNexusUrl(plan.entry.input)
              ? "Waiting for download..."
              : "Starting...",
          );

          // Phases 3+4: Pipelined PAK-check + install, serial via workChain.
          // Track this plan's install futures so we can mark it done the
          // moment its own work finishes, without waiting for other plans.
          const installFutures: Promise<void>[] = [];
          for (const download of plan.downloads) {
            const future = workChain.then(async () => {
              if (!download.promise) return;
              try {
                download.result = await download.promise;
              } catch (error) {
                const msg = String(error);
                if (msg.includes("CANCELLED:")) {
                  modAddQueueStore.markError(plan.entry.queueId, "Cancelled");
                  importLogStore.clear();
                } else {
                  modAddQueueStore.markError(
                    plan.entry.queueId,
                    `Failed: ${msg}`,
                  );
                }
                download.failed = true;
                plan.failed = true;
                return;
              }
              // Safety net: the upfront lookup may have failed (or returned a
              // generic name) - prefer the authoritative name from the result.
              if (download.result?.name) {
                modAddQueueStore.setName(
                  plan.entry.queueId,
                  download.result.name,
                );
              }
              try {
                const result = download.result;
                if (download.failed || !result) return;
                const selectedPaks = await choosePaks(
                  result.archivePath,
                  result.archiveName,
                  plan.entry.queueId,
                );
                if (selectedPaks === null) {
                  modAddQueueStore.markError(plan.entry.queueId, "Cancelled");
                  download.failed = true;
                  return;
                }
                download.selectedPaks = selectedPaks ?? undefined;
                importLogStore.setCurrentMod(plan.entry.queueId);
                modAddQueueStore.markRunning(
                  plan.entry.queueId,
                  "Installing...",
                );
                try {
                  const installResult = await installLocalMod(
                    result.archivePath,
                    download.selectedPaks,
                    result.contentHash,
                  );
                  await updateModDisplayName(
                    result.archiveName,
                    result.name,
                  ).catch(() => {});
                  await updateModSourceUrl(
                    result.archiveName,
                    result.sourceUrl,
                    result.version,
                  ).catch(() => {});
                  if (result.fileId != null) {
                    await updateNexusFileId(
                      result.archiveName,
                      result.fileId,
                    ).catch(() => {});
                  }
                  if (
                    plan.entry.replacingArchiveName &&
                    plan.entry.replacingArchiveName !== result.archiveName
                  ) {
                    await replaceModArchive(
                      plan.entry.replacingArchiveName,
                      result.archiveName,
                    ).catch(() => {});
                  }
                  await applyNexusCategoryTag(
                    result.archiveName,
                    result.category,
                    installResult.wasDuplicate,
                  );
                } catch (error) {
                  modAddQueueStore.markError(
                    plan.entry.queueId,
                    `Failed: ${String(error)}`,
                  );
                  download.failed = true;
                  plan.failed = true;
                }
              } catch (error) {
                modAddQueueStore.markError(
                  plan.entry.queueId,
                  `Failed: ${String(error)}`,
                );
                download.failed = true;
                plan.failed = true;
              }
            });
            installFutures.push(future);
            workChain = future.then(
              () => undefined,
              () => undefined,
            );
          }

          // Mark this plan done as soon as all its installs finish.
          void Promise.all(installFutures).then(() => {
            if (plan.failed) return;
            const succeeded = plan.downloads.filter(
              (d) => d.result && !d.failed,
            );
            if (succeeded.length === 0) return;
            const total = plan.downloads.length;
            const message =
              succeeded.length === total
                ? `Installed ${total} file${total === 1 ? "" : "s"}`
                : `Installed ${succeeded.length} of ${total} files`;
            modAddQueueStore.markDone(plan.entry.queueId, message);
            addModpackPanelStore.notifyModInstalled();
            window.dispatchEvent(new CustomEvent("ron:tags-changed"));
          });
        }

        // Queue drained. Determine what to wait for.
        // Only truly-pending downloads count as in-flight: an already-settled
        // download whose workChain turn hasn't come yet must NOT be raced on
        // (racing a resolved promise resolves immediately and busy-loops,
        // freezing progress rendering for the whole install).
        const inFlight = allPlans
          .flatMap((p) => p.downloads)
          .filter((d) => !d.settled && !d.result && !d.failed)
          .map((d) => d.promise!);

        if (inFlight.length > 0) {
          // Downloads still in flight - wait for any to complete or new items.
          await Promise.race([...inFlight, waitForWake()]);
          if (disarmWake()) {
            // Race settled via a download: the abandoned waiter is now
            // disarmed, but yield to the event loop so Tauri progress events
            // and UI get a chance to run before re-checking.
            await new Promise((r) => setTimeout(r, 0));
          }
          // If new items arrived, loop back to process them.
          continue;
        }

        // All downloads landed. Wait for installs to finish or new items.
        const allDone = allPlans.every((p) =>
          p.downloads.every((d) => d.result || d.failed),
        );
        if (allDone && allPlans.length > 0) {
          await workChain;
          // Re-check the queue before exiting: a late submit during the
          // await above pushed items to pendingLinkQueue but saw
          // isProcessingLinks still true and bailed out. If we set finished
          // here those items would strand with nobody to drain them.
          if (pendingLinkQueue.length > 0) {
            continue;
          }
          finished = true;
        } else if (allPlans.length === 0) {
          // First iteration with nothing queued yet.
          await waitForWake();
          disarmWake();
        } else {
          // Downloads settled but installs pending - wait for work or new items.
          await Promise.race([workChain, waitForWake()]);
          if (disarmWake()) {
            await new Promise((r) => setTimeout(r, 0));
          }
        }
      }
    } finally {
      isProcessingLinks = false;
      disarmWake();
      dispatch("modAdded");
    }
  }

  async function doInstallFile(filePath: string, selectedPakFiles?: string[]) {
    const fileName = filePath.split(/[\\/]/).pop() ?? filePath;
    alertStore.clear();
    const queueId = modAddQueueStore.enqueue(fileName);
    modAddQueueStore.markRunning(queueId, "Installing...");
    try {
      const result = await installLocalMod(filePath, selectedPakFiles);
      if (result.wasDuplicate) {
        modAddQueueStore.markDone(queueId, `${fileName} is already installed`);
        alertStore.info(
          `"${fileName}" is already installed - uninstall it first to reinstall.`,
        );
      } else {
        modAddQueueStore.markDone(queueId, `Installed ${fileName}`);
        addModpackPanelStore.notifyModInstalled();
        dispatch("modAdded");
      }
    } catch (error) {
      modAddQueueStore.markError(queueId, `Failed: ${String(error)}`);
      alertStore.error(String(error));
    }
  }

  async function applyNexusCategoryTag(
    archiveName: string,
    category: string | null | undefined,
    wasDuplicate: boolean,
  ): Promise<void> {
    if (!category || wasDuplicate) return;
    try {
      const tags = await getTags();
      const currentTags = Object.entries(tags)
        .filter(([, mods]) => mods.includes(archiveName))
        .map(([tag]) => tag);
      if (currentTags.includes(category)) return;
      await setModTags(archiveName, [...new Set([...currentTags, category])]);
    } catch {
      // Tagging is best-effort; never fail the install because of it.
    }
  }
  async function choosePaks(
    filePath: string,
    archiveName: string,
    queueId?: string,
  ): Promise<string[] | null | undefined> {
    const ext = filePath.split(".").pop()?.toLowerCase() ?? "";
    if (ext !== "zip" && ext !== "rar" && ext !== "7z") return undefined;
    try {
      const paks = await getArchivePakFiles(filePath);
      if (paks.length <= 1) return undefined;
      if (queueId) {
        modAddQueueStore.markRunning(queueId, "Select PAK files to install...");
        importLogStore.setWaitingForInput(queueId);
      }
      const result = await requestPakSelection(archiveName, paks);
      if (queueId) importLogStore.clearWaitingForInput(queueId);
      return result;
    } catch {
      return undefined;
    }
  }

  async function installFile(filePath: string) {
    const archiveName = filePath.split(/[\\/]/).pop() ?? filePath;
    const selectedPaks = await choosePaks(filePath, archiveName);
    if (selectedPaks === null) return;
    await doInstallFile(filePath, selectedPaks ?? undefined);
  }

  async function handleAddViaFile() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Mod Files", extensions: ["pak", "zip", "rar", "7z"] }],
    });
    if (!selected) return;

    const filePath = Array.isArray(selected) ? selected[0] : selected;
    if (!filePath || typeof filePath !== "string") {
      alertStore.error("No file selected");
      return;
    }

    await installFile(filePath);
  }

  onMount(() => {
    const appWindow = getCurrentWindow();
    void appWindow
      .onDragDropEvent((event) => {
        if (!isVisible || activeTab !== "file") return;
        if (event.payload.type === "over") {
          isDraggingOver = true;
        } else if (event.payload.type === "drop") {
          isDraggingOver = false;
          if ("paths" in event.payload && Array.isArray(event.payload.paths)) {
            for (const path of event.payload.paths) {
              void installFile(path);
            }
          }
        } else if (event.payload.type === "leave") {
          isDraggingOver = false;
        }
      })
      .then((fn) => {
        unlistenDragDrop = fn;
      });
  });

  onDestroy(() => {
    unlistenDragDrop?.();
  });

  function closeModal() {
    modioInput = "";
    activeTab = "link";
    alertStore.clear();
    dispatch("close");
  }
</script>

<!-- AddMod keeps its own drag-drop listeners; only overlay/panel/header
  chrome comes from the shell. -->
<ModalShell
  {isVisible}
  title="Add Mod"
  titleClass="text-2xl font-bold"
  width="w-[560px]"
  closeOnEscape={false}
  on:close={closeModal}
>
  <!-- Tabs -->
  <div
    class="flex gap-2 mb-4 border-b"
    style="border-color: var(--adw-border-color);"
  >
    <button
      on:click={() => {
        activeTab = "link";
        alertStore.clear();
      }}
      style={activeTab === "link"
        ? `color: var(--clr-primary-300); border-bottom: 2px solid var(--clr-primary-300);`
        : `color: var(--clr-text-secondary);`}
      class="pb-2 px-3 text-sm font-medium transition border-b-2 border-transparent cursor-pointer"
    >
      Mod Link
    </button>
    <button
      on:click={() => {
        activeTab = "file";
        alertStore.clear();
      }}
      style={activeTab === "file"
        ? `color: var(--clr-primary-300); border-bottom: 2px solid var(--clr-primary-300);`
        : `color: var(--clr-text-secondary);`}
      class="pb-2 px-3 text-sm font-medium transition border-b-2 border-transparent cursor-pointer"
    >
      Local File
    </button>
  </div>

  <!-- Content area with fixed min-height -->
  <div style="min-height: 180px;">
    {#if activeTab === "link"}
      <div class="space-y-3">
        <div>
          <label
            for="modio-input"
            style="color: var(--clr-text);"
            class="block text-sm font-medium mb-1"
          >
            Mod Links (one per line)
          </label>
          <textarea
            id="modio-input"
            rows="5"
            class="textarea"
            placeholder="https://mod.io/g/readyornot/m/lustful-remorse&#10;https://mod.io/g/readyornot/m/simple-mod-menu&#10;https://www.nexusmods.com/readyornot/mods/1234"
            bind:value={modioInput}
            on:paste={handlePaste}></textarea>
          <p style="color: var(--clr-text-secondary);" class="text-xs mt-1">
            Paste mod.io or Nexus Mods links, one per line
          </p>

          {#if nexusPreviewName}
            <p style="color: var(--clr-success-300);" class="text-xs mt-2">
              Nexus: {nexusPreviewName} - browser will open to download page
            </p>
          {:else if nexusPreviewError}
            <p style="color: var(--clr-danger-300);" class="text-xs mt-2">
              Nexus lookup failed: {nexusPreviewError}
            </p>
          {/if}
        </div>

        {#if activeQueueCount > 0}
          <p style="color: var(--clr-text-secondary);" class="text-xs">
            Running in background: {activeQueueCount}
          </p>
        {/if}

        {#if $alertStore.message}
          <p style={alertStyle} class="text-sm p-2 rounded">
            {$alertStore.message}
          </p>
        {/if}

        <div class="flex gap-2">
          <button on:click={closeModal} class="flex-1 btn"> Cancel </button>
          <button
            on:click={() => handleAddViaLink()}
            disabled={!modioInput.trim()}
            class="flex-1 btn primary"
          >
            Add Mod{parseModInputs(modioInput).length > 1 ? "s" : ""}
          </button>
        </div>
      </div>
    {:else}
      <div class="space-y-3">
        <!-- Drop zone -->
        <button
          on:click={handleAddViaFile}
          style="border-color: {isDraggingOver
            ? 'var(--clr-primary-300)'
            : 'var(--adw-border-color)'}; background: {isDraggingOver
            ? 'color-mix(in srgb, var(--clr-primary-300) 10%, transparent)'
            : 'transparent'};"
          class="w-full rounded-lg border-2 border-dashed p-8 flex flex-col items-center gap-2 cursor-pointer transition-colors"
        >
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="32"
            height="32"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
            style="color: var(--clr-text-secondary);"
          >
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
            <polyline points="17 8 12 3 7 8" />
            <line x1="12" y1="3" x2="12" y2="15" />
          </svg>
          <p style="color: var(--clr-text);" class="text-sm font-medium">
            Drop file here or click to browse
          </p>
          <p style="color: var(--clr-text-secondary);" class="text-xs">
            .pak, .zip, .rar, .7z
          </p>
        </button>

        {#if $alertStore.message}
          <p style={alertStyle} class="text-sm p-2 rounded">
            {$alertStore.message}
          </p>
        {/if}

        <div class="flex gap-2">
          <button on:click={closeModal} class="flex-1 btn"> Cancel </button>
        </div>
      </div>
    {/if}
  </div>
</ModalShell>
