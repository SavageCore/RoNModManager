<script lang="ts">
  import { operationStatusStore } from "$lib/stores/operationStatus";
  import { modAddQueueStore } from "$lib/stores/modAddQueue";

  function formatBytes(value: number): string {
    if (!Number.isFinite(value) || value <= 0) {
      return "0 B";
    }

    const units = ["B", "KiB", "MiB", "GiB"];
    let size = value;
    let unitIndex = 0;

    while (size >= 1024 && unitIndex < units.length - 1) {
      size /= 1024;
      unitIndex += 1;
    }

    return `${size.toFixed(size >= 10 || unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
  }

  $: progressDetails =
    $operationStatusStore.totalBytes &&
    $operationStatusStore.processedBytes != null
      ? `${formatBytes($operationStatusStore.processedBytes)} / ${formatBytes($operationStatusStore.totalBytes)}`
      : "";

  $: activeQueue = $modAddQueueStore.items.filter(
    (item) => item.status === "queued" || item.status === "running",
  );

  $: completedCount = $modAddQueueStore.items.filter(
    (item) => item.status === "done" || item.status === "error",
  ).length;

  $: queueProgress =
    $modAddQueueStore.totalQueued > 0
      ? `${completedCount + (activeQueue.length > 0 ? 1 : 0)}/${$modAddQueueStore.totalQueued}`
      : "";

  $: recentQueue = $modAddQueueStore.items
    .filter((item) => item.status === "done" || item.status === "error")
    .slice(-2)
    .reverse();

  // Auto-reset batch counter when all items complete
  let resetTimeout: ReturnType<typeof setTimeout>;
  $: if (
    $modAddQueueStore.totalQueued > 0 &&
    activeQueue.length === 0 &&
    $modAddQueueStore.items.length > 0 &&
    $modAddQueueStore.items.every(
      (item) => item.status === "done" || item.status === "error",
    )
  ) {
    clearTimeout(resetTimeout);
    resetTimeout = setTimeout(() => {
      modAddQueueStore.resetBatch();
    }, 2000);
  }
</script>

<footer
  style="background: var(--clr-surface); border-top: 1px solid var(--adw-border-color);"
  class="h-9 px-3 flex items-center text-xs gap-4"
>
  {#if activeQueue.length > 0}
    <span style="color: var(--clr-primary-300);" class="shrink-0">
      {queueProgress
        ? `Progress: ${queueProgress}`
        : `Queue: ${activeQueue.length}`}
    </span>
  {/if}

  {#if $operationStatusStore.visible}
    <div class="flex-1 flex items-center gap-3 min-w-0">
      <span
        style={$operationStatusStore.isError
          ? "color: var(--clr-danger-300);"
          : "color: var(--clr-text);"}
        class="truncate"
      >
        {$operationStatusStore.message}
      </span>
      <span style="color: var(--clr-text-secondary);" class="shrink-0">
        {$operationStatusStore.percent.toFixed(0)}%
      </span>
      {#if progressDetails}
        <span style="color: var(--clr-text-secondary);" class="shrink-0">
          {progressDetails}
        </span>
      {/if}
      <div
        class="ml-auto h-1.5 w-28 rounded-full overflow-hidden"
        style="background: var(--clr-surface-variant);"
      >
        <div
          class="h-full transition-all duration-150"
          style={`width: ${$operationStatusStore.percent}%; background: ${$operationStatusStore.isError ? "var(--clr-danger-300)" : "var(--clr-primary-300)"};`}
        ></div>
      </div>
    </div>
  {:else if recentQueue.length > 0}
    <div class="flex-1 min-w-0" style="color: var(--clr-text-secondary);">
      <span class="truncate">
        {recentQueue[0].message}
      </span>
    </div>
  {:else}
    <span style="color: var(--clr-text-secondary);">Idle</span>
  {/if}
</footer>
