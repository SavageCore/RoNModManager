<script lang="ts">
  import { syncLogStore } from "$lib/stores/syncLogStore";
  import LogPanel from "./LogPanel.svelte";
</script>

<LogPanel
  title="Remote Sync"
  isVisible={$syncLogStore.isOpen}
  isLoading={$syncLogStore.isBusy}
  log={$syncLogStore.log}
  logFilename="sync-log"
  on:close={() => syncLogStore.close()}
>
  <svelte:fragment slot="extra-actions">
    <button
      class="btn btn-sm"
      disabled={$syncLogStore.log.length === 0 || $syncLogStore.isBusy}
      on:click={() => syncLogStore.clear()}>Clear</button
    >
  </svelte:fragment>

  {#if $syncLogStore.log.length === 0}
    <div style="color: var(--clr-text-secondary);">Waiting...</div>
  {:else}
    {#each $syncLogStore.log as line}
      <div class="leading-relaxed">{line}</div>
    {/each}
  {/if}
</LogPanel>
