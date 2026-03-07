<script lang="ts">
  import type { ModProgressEvent } from "$lib/types";

  export let isVisible = false;
  export let progress: ModProgressEvent | null = null;
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
  $: isInstalling =
    progress?.operation === "install" || progress?.operation === "extract";
  $: isComplete = progress?.operation === "complete";
</script>

{#if isVisible && progress}
  <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
    <div
      style="background: var(--clr-surface); border-color: var(--adw-border-color);"
      class="border rounded-lg shadow-2xl w-96 p-8"
    >
      <h2 style="color: var(--clr-text);" class="text-2xl font-bold mb-6">
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
        <p style="color: var(--clr-text-secondary);" class="text-sm mb-4">
          {progress.file}
        </p>
      {/if}

      {#if !isComplete && !isError}
        <div class="mb-6">
          <div
            style="background: var(--clr-surface-variant);"
            class="w-full rounded-full h-3 overflow-hidden"
          >
            <div
              class="h-full transition-all duration-300"
              style={`background: var(--clr-primary-300); width: ${Math.min(progress.percent, 100)}%`}
            ></div>
          </div>
          <p
            style="color: var(--clr-text-secondary);"
            class="text-xs mt-2 text-right"
          >
            {Math.round(progress.percent)}%
          </p>
        </div>
      {/if}

      <p style="color: var(--clr-text);" class="text-sm mb-6 h-10">
        {progress.message}
      </p>

      {#if isError}
        <div
          style="background: var(--clr-danger-300); border-color: rgba(0,0,0,0.2);"
          class="border rounded p-3 mb-6"
        >
          <p style="color: var(--clr-danger-text);" class="text-sm">
            {progress.message}
          </p>
        </div>
      {/if}

      {#if isComplete}
        <div
          style="background: var(--clr-primary-300); border-color: rgba(0,0,0,0.2);"
          class="border rounded p-3 mb-6"
        >
          <p style="color: var(--clr-primary-text);" class="text-sm">
            Mods installed successfully!
          </p>
        </div>
      {/if}

      <div class="flex gap-3">
        {#if !isComplete && onCancel}
          <button on:click={onCancel} class="flex-1 btn"> Cancel </button>
        {/if}

        {#if isError || isComplete}
          <button
            on:click={() => {
              isVisible = false;
            }}
            class="flex-1 btn primary"
          >
            Close
          </button>
        {/if}
      </div>
    </div>
  </div>
{/if}
