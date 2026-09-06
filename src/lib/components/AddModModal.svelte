<script lang="ts">
  import {
    addModIoMod,
    addNexusMod,
    fetchNexusModInfo,
    getArchivePakFiles,
    installLocalMod,
    listNexusFileOptions,
    replaceModArchive,
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
        queueId: modAddQueueStore.enqueue(modInput),
        replacingArchiveName,
      });
    }

    // Close immediately - progress is visible in the bottom bar
    closeModal();

    void processQueue();
  }

  // Bulk auto-submit (e.g. "Update All") - each entry replaces its own archive.
  async function submitAutoEntries(
    entries: Array<{
      url: string;
      replacing: string | null;
      displayName?: string;
    }>,
  ) {
    for (const entry of entries) {
      pendingLinkQueue.push({
        input: entry.url,
        queueId: modAddQueueStore.enqueue(entry.url),
        replacingArchiveName: entry.replacing ?? undefined,
        displayName: entry.displayName,
      });
    }
    closeModal();
    void processQueue();
  }

  async function processQueue() {
    // Single worker loop - if already running, the new items will be picked up naturally
    if (isProcessingLinks) return;

    isProcessingLinks = true;
    try {
      while (pendingLinkQueue.length > 0) {
        type Download = {
          promise?:
            ReturnType<typeof addNexusMod> | ReturnType<typeof addModIoMod>;
          result?:
            | Awaited<ReturnType<typeof addNexusMod>>
            | Awaited<ReturnType<typeof addModIoMod>>;
          selectedPaks?: string[];
          failed?: boolean;
        };
        type Plan = {
          entry: {
            input: string;
            queueId: string;
            replacingArchiveName?: string;
            displayName?: string;
          };
          chosenFileIds: number[];
          downloads: Download[];
          installFutures: Promise<void>[];
          failed?: boolean;
        };
        const plans: Plan[] = pendingLinkQueue.splice(0).map((e) => ({
          entry: e,
          chosenFileIds: [],
          downloads: [],
          installFutures: [],
        }));

        // Phase 1: Ask all Nexus file variant questions before downloading anything
        for (const plan of plans) {
          if (!isNexusUrl(plan.entry.input)) continue;
          try {
            modAddQueueStore.markRunning(
              plan.entry.queueId,
              "Checking available files...",
            );
            const fileOptions = await listNexusFileOptions(plan.entry.input);
            let chosenFileIds: number[] = [];
            if (fileOptions.length > 1) {
              modAddQueueStore.markRunning(
                plan.entry.queueId,
                "Select file variant...",
              );
              importLogStore.setWaitingForInput(plan.entry.queueId);
              const chosen = await requestNexusFileSelection(
                plan.entry.displayName || nexusPreviewName || plan.entry.input,
                fileOptions,
              );
              importLogStore.clearWaitingForInput(plan.entry.queueId);
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
          } catch (error) {
            modAddQueueStore.markError(
              plan.entry.queueId,
              `Failed: ${String(error)}`,
            );
            plan.failed = true;
          }
        }

        // Serialises post-download work (PAK picker + install) so prompts never
        // stack and installs don't race each other - while the downloads
        // themselves keep running concurrently in the background.
        let workChain: Promise<void> = Promise.resolve();
        function enqueueWork<T>(fn: () => Promise<T>): Promise<T> {
          const run = workChain.then(fn, fn);
          workChain = run.then(
            () => undefined,
            () => undefined,
          );
          return run;
        }

        // Phase 2: Download all mods. Kick every download off before awaiting any of
        // them, so N Nexus mods open N browser tabs together instead of waiting for
        // each free-account download to land before starting the next.
        // ponytail: concurrent premium downloads share one footer progress bar and
        // will interleave their percentages - same as AddModpackPanel's import flow
        // already does. Upgrade path: branch on checkNexusPremium() and keep premium
        // serial if that ever actually bothers someone.
        for (const plan of plans) {
          if (plan.failed) continue;
          if (isNexusUrl(plan.entry.input)) {
            const fileIds =
              plan.chosenFileIds.length > 0 ? plan.chosenFileIds : [undefined];
            for (const fileId of fileIds) {
              plan.downloads.push({
                promise: addNexusMod(plan.entry.input, fileId),
              });
            }
          } else {
            plan.downloads.push({ promise: addModIoMod(plan.entry.input) });
          }
          modAddQueueStore.markRunning(
            plan.entry.queueId,
            isNexusUrl(plan.entry.input)
              ? "Waiting for download..."
              : "Starting...",
          );
        }

        // Phases 3+4, pipelined per download: the moment an archive lands it is
        // PAK-checked (if needed) and installed, while the remaining downloads
        // are still in flight. No more waiting for the whole queue to finish
        // downloading before anything installs.
        for (const plan of plans) {
          if (plan.failed) continue;
          for (const download of plan.downloads) {
            plan.installFutures.push(
              (async () => {
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
                try {
                  await enqueueWork(async () => {
                    const result = download.result;
                    if (download.failed || !result) return;
                    const selectedPaks = await choosePaks(
                      result.archivePath,
                      result.archiveName,
                      plan.entry.queueId,
                    );
                    if (selectedPaks === null) {
                      modAddQueueStore.markError(
                        plan.entry.queueId,
                        "Cancelled",
                      );
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
                      await installLocalMod(
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
                    } catch (error) {
                      modAddQueueStore.markError(
                        plan.entry.queueId,
                        `Failed: ${String(error)}`,
                      );
                      download.failed = true;
                      plan.failed = true;
                    }
                  });
                } catch (error) {
                  modAddQueueStore.markError(
                    plan.entry.queueId,
                    `Failed: ${String(error)}`,
                  );
                  download.failed = true;
                  plan.failed = true;
                }
              })(),
            );
          }
        }
        // Finalize each plan the moment its own downloads and installs are
        // finished, so queue rows tick green individually instead of all at
        // once when the whole batch completes.
        const completions = plans.map((plan) =>
          Promise.all(plan.installFutures).then(() => {
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
          }),
        );
        await Promise.all(completions);
      }
    } finally {
      isProcessingLinks = false;
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

  // Returns the user's pak selection, undefined (install all) if no choice needed,
  // or null if the user cancelled. Pass queueId to update queue status while waiting.
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
