<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import { addModIoMod, fetchNexusModInfo } from "$lib/api/commands";
  import { alertStore } from "$lib/stores/alert";
  import { modAddQueueStore } from "$lib/stores/modAddQueue";

  export let isVisible = false;

  const dispatch = createEventDispatcher();

  let activeTab: "link" | "file" = "link";
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

  function parseModInputs(input: string): string[] {
    // Split by newlines and commas
    const entries: string[] = [];

    // First split by newlines
    const lines = input.split(/\r?\n/);

    for (const line of lines) {
      const trimmedLine = line.trim();
      if (!trimmedLine || trimmedLine.startsWith("#")) {
        continue; // Skip empty lines and comments
      }

      // Check if line contains commas (CSV format)
      if (trimmedLine.includes(",")) {
        const parts = trimmedLine
          .split(",")
          .map((p) => p.trim())
          .filter((p) => p.length > 0);
        entries.push(...parts);
      } else {
        entries.push(trimmedLine);
      }
    }

    return entries;
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
      alertStore.error("Enter mod.io links, stubs, or mod IDs");
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

    // Process each mod sequentially
    for (const entry of queueEntries) {
      modAddQueueStore.markRunning(entry.queueId, "Starting...");

      try {
        const result = await addModIoMod(entry.input);
        modAddQueueStore.markDone(entry.queueId, `Installed ${result.name}`);
      } catch (error) {
        const message = `Failed: ${String(error)}`;
        modAddQueueStore.markError(entry.queueId, message);
      }
    }

    dispatch("modAdded");
  }

  async function handleAddViaFile() {
    // TODO: Implement file picker and upload
    alertStore.info("File upload coming soon...");
  }

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
      class="border rounded-lg shadow-2xl w-96 p-6"
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
          on:click={() => (activeTab = "link")}
          style={activeTab === "link"
            ? `color: var(--clr-primary-300); border-bottom: 2px solid var(--clr-primary-300);`
            : `color: var(--clr-text-secondary);`}
          class="pb-2 px-3 text-sm font-medium transition border-b-2 border-transparent cursor-pointer"
        >
          mod.io Link
        </button>
        <button
          on:click={() => (activeTab = "file")}
          style={activeTab === "file"
            ? `color: var(--clr-primary-300); border-bottom: 2px solid var(--clr-primary-300);`
            : `color: var(--clr-text-secondary);`}
          class="pb-2 px-3 text-sm font-medium transition border-b-2 border-transparent cursor-pointer"
        >
          Upload File
        </button>
      </div>

      <!-- Content area with fixed min-height -->
      <div style="min-height: 280px;">
        {#if activeTab === "link"}
          <div class="space-y-3">
            <div>
              <label
                for="modio-input"
                style="color: var(--clr-text);"
                class="block text-sm font-medium mb-1"
              >
                mod.io Links or IDs (one per line or comma-separated)
              </label>
              <textarea
                id="modio-input"
                rows="5"
                class="textarea"
                placeholder="https://mod.io/g/readyornot/m/mod-name&#10;another-mod-name&#10;or, comma, separated"
                bind:value={modioInput}
              ></textarea>
              <p style="color: var(--clr-text-secondary);" class="text-xs mt-1">
                Paste mod.io links, stubs, or IDs (one per line or
                comma-separated)
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
            <div>
              <label
                for="file-input"
                style="color: var(--clr-text);"
                class="block text-sm font-medium mb-2"
              >
                Select .pak or Archive File
              </label>
              <input
                id="file-input"
                type="file"
                accept=".pak,.zip,.rar,.7z"
                on:change={handleAddViaFile}
                class="w-full"
              />
              <p style="color: var(--clr-text-secondary);" class="text-xs mt-1">
                Supported: .pak files and archives (.zip, .rar, .7z)
              </p>
            </div>

            {#if $alertStore.message}
              <p style={alertStyle} class="text-sm p-2 rounded">
                {$alertStore.message}
              </p>
            {/if}

            <div class="flex gap-2">
              <button on:click={closeModal} class="flex-1 btn"> Cancel </button>
              <button class="flex-1 btn primary"> Upload </button>
            </div>
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}
