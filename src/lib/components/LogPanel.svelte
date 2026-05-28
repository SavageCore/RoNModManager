<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import Minus from "lucide-svelte/icons/minus";

  export let title: string;
  export let isVisible = true;
  export let isLoading = false;
  export let log: string[] = [];
  export let width = "420px";
  export let maxHeight = "400px";
  export let logFilename = "log";

  const dispatch = createEventDispatcher<{ close: void; clear: void }>();

  function autoScroll(node: HTMLElement) {
    const observer = new MutationObserver(() => {
      node.scrollTop = node.scrollHeight;
    });
    observer.observe(node, { childList: true, subtree: true });
    return { destroy: () => observer.disconnect() };
  }

  function copyLog() {
    navigator.clipboard.writeText(log.join("\n")).catch(() => {});
  }

  function saveLog() {
    const ts = new Date().toISOString().replace(/[:.]/g, "-").slice(0, 19);
    const blob = new Blob([log.join("\n")], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${logFilename}-${ts}.txt`;
    a.click();
    URL.revokeObjectURL(url);
  }
</script>

{#if isVisible}
  <div
    class="fixed right-4 z-[900] flex flex-col rounded-lg border shadow-xl"
    style="bottom: calc(2.25rem + 0.5rem); width: {width}; max-height: {maxHeight}; background: var(--clr-surface); border-color: var(--adw-border-color);"
  >
    <div
      class="flex items-center justify-between px-3 py-2 border-b shrink-0"
      style="border-color: var(--adw-border-color);"
    >
      <span class="text-sm font-medium" style="color: var(--clr-text);"
        >{title}</span
      >
      <button
        class="h-6 w-6 flex items-center justify-center rounded"
        style="color: var(--clr-text-secondary);"
        on:click={() => dispatch("close")}
        aria-label="Minimise"><Minus size={14} /></button
      >
    </div>

    <slot name="controls" />

    <div
      use:autoScroll
      class="overflow-y-auto flex-1 mx-3 mb-2 mt-2 p-2 rounded text-xs font-mono"
      style="background: var(--clr-surface-variant, var(--adw-dark-fill-color, #1e1e1e)); color: var(--clr-text);"
    >
      <slot />
    </div>

    <div class="flex justify-end gap-2 px-3 pb-3 shrink-0">
      <slot name="extra-actions" />
      <button class="btn btn-sm" disabled={log.length === 0} on:click={copyLog}
        >Copy</button
      >
      <button class="btn btn-sm" disabled={log.length === 0} on:click={saveLog}
        >Save</button
      >
      <button
        class="btn btn-sm"
        disabled={isLoading}
        on:click={() => dispatch("clear")}>Close</button
      >
    </div>
  </div>
{/if}
