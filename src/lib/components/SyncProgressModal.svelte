<script lang="ts">
  import { afterUpdate, createEventDispatcher } from "svelte";

  export let isVisible = false;
  export let log: string[] = [];
  export let isBusy = false;

  const dispatch = createEventDispatcher<{ close: void }>();

  let logDiv: HTMLDivElement | null = null;

  afterUpdate(() => {
    if (logDiv) {
      logDiv.scrollTop = logDiv.scrollHeight;
    }
  });

  function copyLog() {
    navigator.clipboard.writeText(log.join("\n")).catch(() => {});
  }

  function saveLog() {
    const ts = new Date().toISOString().replace(/[:.]/g, "-").slice(0, 19);
    const blob = new Blob([log.join("\n")], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `sync-log-${ts}.txt`;
    a.click();
    URL.revokeObjectURL(url);
  }

  function handleClose() {
    dispatch("close");
  }
</script>

{#if isVisible}
  <div
    class="fixed inset-0 z-[1100] flex items-center justify-center bg-black/50"
  >
    <div
      class="w-[520px] max-w-[92vw] rounded-lg border p-6 shadow-2xl bg-[var(--clr-surface)] border-[var(--adw-border-color)]"
    >
      <h2 class="text-lg font-semibold mb-1" style="color: var(--clr-text);">
        Remote Sync
      </h2>
      <p class="text-sm mb-4" style="color: var(--clr-text-secondary);">
        {isBusy ? "Syncing files to remote server..." : "Sync complete."}
      </p>

      <div
        bind:this={logDiv}
        class="rounded p-2 text-xs h-48 overflow-y-auto mb-4 font-mono"
        style="background: var(--clr-surface-variant, var(--adw-dark-fill-color, #1e1e1e)); color: var(--clr-text);"
      >
        {#each log as line}
          <div class="leading-relaxed">{line}</div>
        {/each}
        {#if log.length === 0}
          <div style="color: var(--clr-text-secondary);">Waiting...</div>
        {/if}
      </div>

      <div class="flex justify-end gap-2">
        <button
          class="btn btn-sm"
          disabled={log.length === 0}
          on:click={copyLog}
        >
          Copy Log
        </button>
        <button
          class="btn btn-sm"
          disabled={log.length === 0}
          on:click={saveLog}
        >
          Save Log
        </button>
        <button
          class="btn btn-sm primary"
          disabled={isBusy}
          on:click={handleClose}
        >
          Close
        </button>
      </div>
    </div>
  </div>
{/if}
