<script lang="ts">
  import { infoLogStore } from "$lib/stores/infoLogStore";
  import LogPanel from "./LogPanel.svelte";

  $: lineColor =
    $infoLogStore.tone === "error"
      ? "var(--clr-danger-300)"
      : $infoLogStore.tone === "success"
        ? "var(--clr-success)"
        : "var(--clr-text)";
</script>

<LogPanel
  title="Metadata Refresh"
  isVisible={$infoLogStore.isOpen}
  isLoading={$infoLogStore.isBusy}
  log={$infoLogStore.lines}
  logFilename="metadata-refresh-log"
  on:close={() => infoLogStore.close()}
>
  <svelte:fragment slot="extra-actions">
    <button
      class="btn btn-sm"
      disabled={$infoLogStore.lines.length === 0 || $infoLogStore.isBusy}
      on:click={() => infoLogStore.clear()}>Clear</button
    >
  </svelte:fragment>

  {#if $infoLogStore.lines.length === 0}
    <div style="color: var(--clr-text-secondary);">Waiting...</div>
  {:else}
    {#each $infoLogStore.lines as line}
      <div class="leading-relaxed" style="color: {lineColor};">{line}</div>
    {/each}
  {/if}
</LogPanel>
