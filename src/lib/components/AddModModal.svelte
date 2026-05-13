<script lang="ts">
  import { createEventDispatcher, onMount, onDestroy } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import {
    addModIoMod,
    fetchNexusModInfo,
    installLocalMod,
  } from "$lib/api/commands";
  import { alertStore } from "$lib/stores/alert";
  import { modAddQueueStore } from "$lib/stores/modAddQueue";

  export let isVisible = false;

  const dispatch = createEventDispatcher();

  let activeTab: "link" | "file" = "link";
  let isDraggingOver = false;
  let unlistenDragDrop: (() => void) | null = null;
  let modioInput = "";
  let nexusPreviewName = "";
  let nexusPreviewError = "";
  let nexusLookupTimer: ReturnType<typeof setTimeout> | null = null;
  let nexusLookupToken = 0;

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

  async function handleAddViaLink() {
    const input = modioInput.trim();
    if (!input) {
      alertStore.error("Enter mod.io links");
      return;
    }

    const modInputs = parseModInputs(input);

    if (modInputs.length === 0) {
      alertStore.error("No valid mod inputs found");
      return;
    }

    alertStore.clear();
    modioInput = "";

    // Reset batch counter for new queue
    modAddQueueStore.resetBatch();

    // Enqueue all mods
    const queueEntries = modInputs.map((modInput) => ({
      input: modInput,
      queueId: modAddQueueStore.enqueue(modInput),
    }));

    alertStore.info(
      `Queued ${queueEntries.length} mod${queueEntries.length > 1 ? "s" : ""} in background...`,
    );

    let successCount = 0;
    let failureCount = 0;

    // Process each mod sequentially
    for (const entry of queueEntries) {
      modAddQueueStore.markRunning(entry.queueId, "Starting...");

      try {
        const result = await addModIoMod(entry.input);
        modAddQueueStore.markDone(entry.queueId, `Installed ${result.name}`);
        successCount += 1;
      } catch (error) {
        const message = `Failed: ${String(error)}`;
        modAddQueueStore.markError(entry.queueId, message);
        failureCount += 1;
      }
    }

    if (failureCount === 0) {
      alertStore.success(
        `Finished: installed ${successCount} mod${successCount === 1 ? "" : "s"}.`,
      );
    } else if (successCount === 0) {
      alertStore.error(
        `Finished: all ${failureCount} mod${failureCount === 1 ? "" : "s"} failed.`,
      );
    } else {
      alertStore.info(
        `Finished: installed ${successCount}, failed ${failureCount}.`,
      );
    }

    dispatch("modAdded");
  }

  async function installFile(filePath: string) {
    const fileName = filePath.split(/[\\/]/).pop() ?? filePath;
    alertStore.clear();
    const queueId = modAddQueueStore.enqueue(fileName);
    modAddQueueStore.markRunning(queueId, "Installing...");
    try {
      const result = await installLocalMod(filePath);
      if (result.wasDuplicate) {
        modAddQueueStore.markDone(queueId, `${fileName} is already installed`);
        alertStore.info(
          `"${fileName}" is already installed — uninstall it first to reinstall.`,
        );
      } else {
        modAddQueueStore.markDone(queueId, `Installed ${fileName}`);
        dispatch("modAdded");
      }
    } catch (error) {
      modAddQueueStore.markError(queueId, `Failed: ${String(error)}`);
      alertStore.error(String(error));
    }
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

{#if isVisible}
  <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
    <div
      style="background: var(--clr-surface); border-color: var(--adw-border-color);"
      class="border rounded-lg shadow-2xl w-[560px] p-6"
    >
      <div class="flex items-center justify-between mb-4">
        <h2 style="color: var(--clr-text);" class="text-2xl font-bold">
          Add Mod
        </h2>
        <button
          on:click={closeModal}
          style="color: var(--clr-text-secondary);"
          class="text-2xl hover:opacity-70 transition cursor-pointer"
        >
          ×
        </button>
      </div>

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
          mod.io Link
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
                mod.io Links (one per line)
              </label>
              <textarea
                id="modio-input"
                rows="5"
                class="textarea"
                placeholder="https://mod.io/g/readyornot/m/lustful-remorse&#10;https://mod.io/g/readyornot/m/simple-mod-menu&#10;https://mod.io/g/readyornot/m/uon-official"
                bind:value={modioInput}
                on:paste={handlePaste}
              ></textarea>
              <p style="color: var(--clr-text-secondary);" class="text-xs mt-1">
                Paste mod.io links, one per line
              </p>

              {#if nexusPreviewName}
                <p style="color: var(--clr-success-300);" class="text-xs mt-2">
                  Nexus detected: {nexusPreviewName}
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
                on:click={handleAddViaLink}
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
    </div>
  </div>
{/if}
