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

  // Batch mode: multiple mods queued and not all finished yet
  $: stillInBatch =
    $modAddQueueStore.totalQueued > 1 &&
    completedCount < $modAddQueueStore.totalQueued;

  // When bytes are available, derive percent directly from bytes so it matches the displayed sizes.
  // Otherwise normalize backend phase-mapped percents so task text shows 0-100 per phase.
  $: bytePercent =
    $operationStatusStore.totalBytes != null &&
    $operationStatusStore.totalBytes > 0 &&
    $operationStatusStore.processedBytes != null
      ? Math.min(
          100,
          ($operationStatusStore.processedBytes /
            $operationStatusStore.totalBytes) *
            100,
        )
      : null;

  function mapRangeToPercent(
    value: number,
    start: number,
    end: number,
  ): number {
    if (end <= start) {
      return 0;
    }
    const clamped = Math.min(end, Math.max(start, value));
    return ((clamped - start) / (end - start)) * 100;
  }

  function normalizeTaskPercent(operation: string, percent: number): number {
    const op = operation || "";

    if (op.includes("download")) {
      // add_modio_mod backend maps download to 5-50.
      return mapRangeToPercent(percent, 5, 50);
    }

    if (op === "hash") {
      // install_local_mod backend maps hashing to 8-40.
      return mapRangeToPercent(percent, 8, 40);
    }

    if (op === "dedupe") {
      // "Checking cache" / dedupe is a short phase around 40-98.
      return mapRangeToPercent(percent, 40, 98);
    }

    if (op === "extract") {
      // Archive extraction progress is emitted in the 55-95 range.
      return mapRangeToPercent(percent, 55, 95);
    }

    return Math.max(0, Math.min(100, percent));
  }

  $: rawPercent =
    bytePercent ??
    normalizeTaskPercent(
      $operationStatusStore.operation,
      $operationStatusStore.percent,
    );

  // Monotonically increasing per-phase text percent — never goes backwards within a phase.
  let smoothModPercent = 0;
  let _lastCompletedCount = 0;
  let _lastOperation = "";
  let _lastFile = "";

  // Monotonically increasing per-file percent for the batch bar.
  // This intentionally ignores phase resets so the right-hand bar stays stable.
  let smoothBatchItemPercent = 0;

  $: {
    const completedChanged = completedCount !== _lastCompletedCount;

    if (completedChanged) {
      _lastCompletedCount = completedCount;
      smoothModPercent = 0;
      // A queue item finished; next item should start from 0 for its own slot.
      smoothBatchItemPercent = 0;
    }

    const operationChanged = $operationStatusStore.operation !== _lastOperation;
    const fileChanged = $operationStatusStore.file !== _lastFile;

    if (operationChanged || fileChanged) {
      smoothModPercent = 0;
      _lastOperation = $operationStatusStore.operation;
      _lastFile = $operationStatusStore.file;
    }

    if (!$operationStatusStore.visible && !suppressComplete) {
      smoothModPercent = 0;
      smoothBatchItemPercent = 0;
      _lastOperation = "";
      _lastFile = "";
    }

    const isPerModCompleteInBatch =
      $operationStatusStore.operation === "complete" && stillInBatch;
    if (!isPerModCompleteInBatch && rawPercent > smoothModPercent) {
      smoothModPercent = rawPercent;
    }

    // For batch tracking, use backend raw percent directly (phase-weighted timeline).
    // Do not use byte-derived percent here; that would mark download as 100 too early.
    const batchRawPercent = isPerModCompleteInBatch
      ? 0
      : Math.max(0, Math.min(100, $operationStatusStore.percent));
    if (batchRawPercent > smoothBatchItemPercent) {
      smoothBatchItemPercent = batchRawPercent;
    }
  }

  // Text percent should track only the current task (0-100).
  $: taskPercent = smoothModPercent;

  // Overall progress: each completed mod contributes its equal share; current mod fills its slot
  $: batchPercent = stillInBatch
    ? Math.min(
        100,
        (completedCount / $modAddQueueStore.totalQueued) * 100 +
          smoothBatchItemPercent / $modAddQueueStore.totalQueued,
      )
    : smoothBatchItemPercent;

  // Blue bar tracks overall batch progress.
  $: displayPercent = batchPercent;

  // Suppress per-mod "complete" flash; keep showing progress until the batch finishes
  $: suppressComplete =
    stillInBatch && $operationStatusStore.operation === "complete";

  $: showOperationStatus = $operationStatusStore.visible || suppressComplete;

  // Detect when download bytes are fully received but backend hasn't sent the next event yet.
  // We delay showing "Processing download..." by 600ms so the user sees 100% first.
  $: _downloadBytesComplete =
    $operationStatusStore.operation?.includes("download") &&
    $operationStatusStore.totalBytes != null &&
    $operationStatusStore.totalBytes > 0 &&
    $operationStatusStore.processedBytes != null &&
    $operationStatusStore.processedBytes >= $operationStatusStore.totalBytes;

  let isDownloadComplete = false;
  let downloadCompleteTimer: ReturnType<typeof setTimeout> | null = null;

  $: {
    if (_downloadBytesComplete) {
      if (!downloadCompleteTimer) {
        downloadCompleteTimer = setTimeout(() => {
          isDownloadComplete = true;
          downloadCompleteTimer = null;
        }, 600);
      }
    } else {
      if (downloadCompleteTimer) {
        clearTimeout(downloadCompleteTimer);
        downloadCompleteTimer = null;
      }
      isDownloadComplete = false;
    }
  }

  $: displayMessage = suppressComplete
    ? `Installing... (${queueProgress})`
    : isDownloadComplete
      ? "Processing download..."
      : $operationStatusStore.message;

  // During the download-to-processing handoff, clear the determinate fill and
  // show only an indeterminate animation. When real progress resumes, the
  // monotonic batch state restores the bar position.
  $: showIndeterminateBar = isDownloadComplete;

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

  {#if showOperationStatus}
    <div class="flex-1 flex items-center gap-3 min-w-0">
      <span
        style={$operationStatusStore.isError
          ? "color: var(--clr-danger-300);"
          : "color: var(--clr-text);"}
        class="truncate"
      >
        {displayMessage}
      </span>
      {#if !isDownloadComplete && !suppressComplete}
        <span style="color: var(--clr-text-secondary);" class="shrink-0">
          {taskPercent.toFixed(0)}%
        </span>
      {/if}
      {#if progressDetails && !suppressComplete && !isDownloadComplete}
        <span style="color: var(--clr-text-secondary);" class="shrink-0">
          {progressDetails}
        </span>
      {/if}
      <div
        class="progress-track ml-auto h-1.5 w-28 rounded-full overflow-hidden"
        style="background: var(--clr-surface-variant);"
      >
        {#if showIndeterminateBar}
          <div class="progress-overlay">
            <div
              class="h-full indeterminate-bar"
              style="background: var(--clr-primary-300);"
            ></div>
          </div>
        {:else}
          <div
            class="progress-fill h-full transition-all duration-150"
            style={`width: ${displayPercent}%; background: ${$operationStatusStore.isError ? "var(--clr-danger-300)" : "var(--clr-primary-300)"};`}
          ></div>
        {/if}
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

<style>
  @keyframes indeterminate {
    0% {
      transform: translateX(-100%);
      width: 50%;
    }
    100% {
      transform: translateX(250%);
      width: 50%;
    }
  }

  .indeterminate-bar {
    animation: indeterminate 1.2s ease-in-out infinite;
  }

  .progress-track {
    position: relative;
  }

  .progress-fill {
    position: absolute;
    inset: 0 auto 0 0;
  }

  .progress-overlay {
    position: absolute;
    inset: 0;
  }
</style>
