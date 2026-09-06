<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import {
    X,
    AlertCircle,
    EyeOff,
    SkipForward,
    Copy,
    Save,
    Check,
  } from "lucide-svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import { writeTextFile } from "$lib/api/commands";

  export let isVisible = false;
  export let skippedMods: { name: string; reason: string }[] = [];
  export let hiddenMods: { name: string; reason: string }[] = [];
  export let failedMods: { name: string; reason: string }[] = [];

  $: safeSkippedMods = skippedMods ?? [];
  $: safeHiddenMods = hiddenMods ?? [];
  $: safeFailedMods = failedMods ?? [];

  const dispatch = createEventDispatcher<{ close: void }>();

  let copyTooltip = "Copy details";
  let saveTooltip = "Save details";

  function handleClose() {
    dispatch("close");
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") handleClose();
  }

  function buildDetailText(): string {
    const lines: string[] = [];
    lines.push("Metadata Refresh Details");
    lines.push("=".repeat(30));
    lines.push("");

    if (safeFailedMods.length > 0) {
      lines.push(`FAILED (${safeFailedMods.length}):`);
      for (const mod of safeFailedMods) {
        lines.push(`  - ${mod.name}: ${mod.reason}`);
      }
      lines.push("");
    }

    if (safeHiddenMods.length > 0) {
      lines.push(`HIDDEN (${safeHiddenMods.length}):`);
      for (const mod of safeHiddenMods) {
        lines.push(`  - ${mod.name}: ${mod.reason}`);
      }
      lines.push("");
    }

    if (safeSkippedMods.length > 0) {
      lines.push(`SKIPPED (${safeSkippedMods.length}):`);
      for (const mod of safeSkippedMods) {
        lines.push(`  - ${mod.name}: ${mod.reason}`);
      }
      lines.push("");
    }

    if (
      safeFailedMods.length === 0 &&
      safeHiddenMods.length === 0 &&
      safeSkippedMods.length === 0
    ) {
      lines.push("No skipped or failed mods.");
    }

    return lines.join("\n");
  }

  async function copyWithFallback(text: string): Promise<boolean> {
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch {
      // Clipboard API can fail in webviews - fall back to execCommand.
      try {
        const area = document.createElement("textarea");
        area.value = text;
        area.style.position = "fixed";
        area.style.opacity = "0";
        document.body.appendChild(area);
        area.select();
        const ok = document.execCommand("copy");
        document.body.removeChild(area);
        return ok;
      } catch {
        return false;
      }
    }
  }

  async function handleCopy() {
    const ok = await copyWithFallback(buildDetailText());
    copyTooltip = ok ? "Copied!" : "Copy failed";
    setTimeout(() => {
      copyTooltip = "Copy details";
    }, 1500);
  }

  async function handleSave() {
    try {
      const text = buildDetailText();
      const filePath = await save({
        defaultPath: `metadata-refresh-details-${new Date().toISOString().slice(0, 19).replace(/:/g, "-")}.txt`,
        filters: [{ name: "Text Files", extensions: ["txt"] }],
      });
      if (filePath) {
        await writeTextFile(filePath, text);
        saveTooltip = "Saved!";
      }
    } catch {
      saveTooltip = "Save failed";
    }
    setTimeout(() => {
      saveTooltip = "Save details";
    }, 1500);
  }
</script>

{#if isVisible}
  <div
    class="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    on:keydown={handleKeydown}
  >
    <div
      style="background: var(--clr-surface); border-color: var(--adw-border-color);"
      class="border rounded-lg shadow-2xl w-[32rem] max-h-[80vh] flex flex-col"
    >
      <div
        class="flex items-center justify-between p-4 border-b"
        style="border-color: var(--adw-border-color);"
      >
        <h2 style="color: var(--clr-text);" class="text-lg font-semibold">
          Metadata Refresh Details
        </h2>
        <div class="flex items-center gap-2">
          <button
            on:click={handleCopy}
            class="p-2 rounded hover:bg-white/10 transition-colors"
            style="color: var(--clr-text-secondary);"
            aria-label={copyTooltip}
            title={copyTooltip}
          >
            <Copy size={18} />
          </button>
          <button
            on:click={handleSave}
            class="p-2 rounded hover:bg-white/10 transition-colors"
            style="color: var(--clr-text-secondary);"
            aria-label={saveTooltip}
            title={saveTooltip}
          >
            <Save size={18} />
          </button>
          <button
            on:click={handleClose}
            class="p-1 rounded hover:bg-white/10 transition-colors"
            style="color: var(--clr-text-secondary);"
            aria-label="Close"
          >
            <X size={20} />
          </button>
        </div>
      </div>

      <div class="p-4 overflow-y-auto">
        {#if safeFailedMods.length > 0}
          <div class="mb-6">
            <div class="flex items-center gap-2 mb-2">
              <AlertCircle size={16} style="color: var(--clr-danger-300);" />
              <h3 style="color: var(--clr-danger-300);" class="font-medium">
                Failed ({safeFailedMods.length})
              </h3>
            </div>
            <ul class="space-y-1">
              {#each safeFailedMods as mod}
                <li class="text-sm" style="color: var(--clr-text);">
                  <span class="font-medium">{mod.name}</span>
                  <span
                    class="text-xs"
                    style="color: var(--clr-text-secondary);"
                  >
                    - {mod.reason}
                  </span>
                </li>
              {/each}
            </ul>
          </div>
        {/if}

        {#if safeHiddenMods.length > 0}
          <div class="mb-6">
            <div class="flex items-center gap-2 mb-2">
              <EyeOff size={16} style="color: var(--clr-text-secondary);" />
              <h3 style="color: var(--clr-text-secondary);" class="font-medium">
                Hidden ({safeHiddenMods.length})
              </h3>
            </div>
            <ul class="space-y-1">
              {#each safeHiddenMods as mod}
                <li class="text-sm" style="color: var(--clr-text);">
                  <span class="font-medium">{mod.name}</span>
                  <span
                    class="text-xs"
                    style="color: var(--clr-text-secondary);"
                  >
                    - {mod.reason}
                  </span>
                </li>
              {/each}
            </ul>
          </div>
        {/if}

        {#if safeSkippedMods.length > 0}
          <div>
            <div class="flex items-center gap-2 mb-2">
              <SkipForward
                size={16}
                style="color: var(--clr-text-secondary);"
              />
              <h3 style="color: var(--clr-text-secondary);" class="font-medium">
                Skipped ({safeSkippedMods.length})
              </h3>
            </div>
            <ul class="space-y-1">
              {#each safeSkippedMods as mod}
                <li class="text-sm" style="color: var(--clr-text);">
                  <span class="font-medium">{mod.name}</span>
                  <span
                    class="text-xs"
                    style="color: var(--clr-text-secondary);"
                  >
                    - {mod.reason}
                  </span>
                </li>
              {/each}
            </ul>
          </div>
        {/if}

        {#if safeFailedMods.length === 0 && safeHiddenMods.length === 0 && safeSkippedMods.length === 0}
          <p class="text-center" style="color: var(--clr-text-secondary);">
            No skipped or failed mods.
          </p>
        {/if}
      </div>

      <div
        class="flex justify-end gap-2 p-4 border-t"
        style="border-color: var(--adw-border-color);"
      >
        <button class="btn primary" on:click={handleClose}>
          <Check size={16} class="mr-1" />
          Close
        </button>
      </div>
    </div>
  </div>
{/if}
