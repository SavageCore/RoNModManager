<script lang="ts">
  import { infoLogStore } from "$lib/stores/infoLogStore";
  import {
    buildMetadataRefreshDetailText,
    metadataRefreshDetailsStore,
  } from "$lib/stores/metadataRefreshDetailsStore";
  import LogPanel from "./LogPanel.svelte";
  import MetadataRefreshDetailsModal from "./MetadataRefreshDetailsModal.svelte";

  let showDetails = false;

  function handleShowDetails() {
    showDetails = true;
    infoLogStore.close();
  }

  $: lineColor =
    $infoLogStore.tone === "error"
      ? "var(--clr-danger-300)"
      : $infoLogStore.tone === "success"
        ? "var(--clr-success)"
        : "var(--clr-text)";

  $: detailText = buildMetadataRefreshDetailText(
    $metadataRefreshDetailsStore.skipped,
    $metadataRefreshDetailsStore.failed,
    $metadataRefreshDetailsStore.hidden,
  );
  $: hasDetails = detailText.length > 0;
  $: fullText =
    $infoLogStore.lines.length > 0 && hasDetails
      ? `${$infoLogStore.lines.join("\n")}\n\n${detailText}`
      : $infoLogStore.lines.join("\n");
</script>

<LogPanel
  title="Metadata Refresh"
  isVisible={$infoLogStore.isOpen}
  isLoading={$infoLogStore.isBusy}
  log={$infoLogStore.lines}
  {fullText}
  logFilename="metadata-refresh-log"
  on:close={() => infoLogStore.close()}
  on:clear={() => infoLogStore.clear()}
>
  {#if $infoLogStore.lines.length === 0}
    <div style="color: var(--clr-text-secondary);">Waiting...</div>
  {:else}
    {#each $infoLogStore.lines as line}
      <div class="leading-relaxed" style="color: {lineColor};">{line}</div>
    {/each}
  {/if}

  <svelte:fragment slot="extra-actions">
    {#if hasDetails}
      <button class="btn btn-sm" on:click={handleShowDetails}>Details</button>
    {/if}
  </svelte:fragment>
</LogPanel>

<MetadataRefreshDetailsModal
  isVisible={showDetails}
  skippedMods={$metadataRefreshDetailsStore.skipped}
  hiddenMods={$metadataRefreshDetailsStore.hidden}
  failedMods={$metadataRefreshDetailsStore.failed}
  on:close={() => {
    showDetails = false;
  }}
/>
