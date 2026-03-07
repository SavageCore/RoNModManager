<script lang="ts">
  import type { ProgressEvent } from "$lib/types";

  export let isVisible = false;
  export let progress: ProgressEvent | null = null;
  export let onCancel: (() => void) | null = null;

  let autoHideTimer: ReturnType<typeof setTimeout> | null = null;

  function handleProgressChange() {
    if (progress?.operation === "complete") {
      // Auto-hide after 2 seconds on success
      autoHideTimer = setTimeout(() => {
        isVisible = false;
      }, 2000);
    } else if (autoHideTimer) {
      // Clear auto-hide timer if progress changes from complete
      clearTimeout(autoHideTimer);
      autoHideTimer = null;
    }
  }

  $: if (progress) {
    handleProgressChange();
  }

  $: isError = progress?.operation === "error";
  $: isFetching = progress?.operation === "fetch";
  $: isDownloading =
    progress?.operation === "download_start" ||
    progress?.operation?.includes("download");
  $: isInstalling = progress?.operation === "install";
  $: isComplete = progress?.operation === "complete";
</script>

{#if isVisible && progress}
  <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
    <div class="bg-white dark:bg-slate-800 rounded-lg shadow-2xl w-96 p-8">
      <h2 class="text-2xl font-bold mb-6 text-slate-900 dark:text-white">
        {#if isFetching}
          Fetching Modpack
        {:else if isDownloading}
          Downloading Mods
        {:else if isInstalling}
          Installing Mods
        {:else if isComplete}
          Complete!
        {:else if isError}
          Installation Error
        {:else}
          Installing...
        {/if}
      </h2>

      {#if progress.file}
        <p class="text-sm text-slate-600 dark:text-slate-300 mb-4">
          {progress.file}
        </p>
      {/if}

      {#if !isComplete && !isError}
        <div class="mb-6">
          <div
            class="w-full bg-slate-200 dark:bg-slate-700 rounded-full h-3 overflow-hidden"
          >
            <div
              class="bg-blue-500 h-full transition-all duration-300"
              style={`width: ${Math.min(progress.percent, 100)}%`}
            ></div>
          </div>
          <p class="text-xs text-slate-600 dark:text-slate-400 mt-2 text-right">
            {Math.round(progress.percent)}%
          </p>
        </div>
      {/if}

      <p class="text-sm text-slate-700 dark:text-slate-200 mb-6 h-10">
        {progress.message}
      </p>

      {#if isError}
        <div
          class="bg-red-100 dark:bg-red-900/30 border border-red-300 dark:border-red-700 rounded p-3 mb-6"
        >
          <p class="text-sm text-red-800 dark:text-red-200">
            {progress.message}
          </p>
        </div>
      {/if}

      {#if isComplete}
        <div
          class="bg-green-100 dark:bg-green-900/30 border border-green-300 dark:border-green-700 rounded p-3 mb-6"
        >
          <p class="text-sm text-green-800 dark:text-green-200">
            Mods installed successfully!
          </p>
        </div>
      {/if}

      <div class="flex gap-3">
        {#if !isComplete && onCancel}
          <button
            on:click={onCancel}
            class="flex-1 px-4 py-2 bg-slate-300 dark:bg-slate-700 text-slate-900 dark:text-white rounded hover:bg-slate-400 dark:hover:bg-slate-600 transition"
          >
            Cancel
          </button>
        {/if}

        {#if isError || isComplete}
          <button
            on:click={() => {
              isVisible = false;
            }}
            class="flex-1 px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 transition"
          >
            Close
          </button>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  :global(.no-scroll) {
    overflow: hidden;
  }
</style>
