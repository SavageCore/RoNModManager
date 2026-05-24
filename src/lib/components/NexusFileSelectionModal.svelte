<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import type { NexusFileOption } from "$lib/api/commands";

  export let modName: string = "";
  export let files: NexusFileOption[] = [];

  const dispatch = createEventDispatcher<{
    select: NexusFileOption;
    cancel: void;
  }>();

  let selected: NexusFileOption = files[0];

  function handleDownload() {
    dispatch("select", selected);
  }

  function handleCancel() {
    dispatch("cancel");
  }
</script>

<div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
  <div
    style="background: var(--clr-surface); border-color: var(--adw-border-color);"
    class="border rounded-lg shadow-2xl w-[520px] p-6"
  >
    <div class="flex items-center justify-between mb-2">
      <h2 style="color: var(--clr-text);" class="text-xl font-bold">
        Select file variant
      </h2>
      <button
        on:click={handleCancel}
        style="color: var(--clr-text-secondary);"
        class="text-2xl hover:opacity-70 transition cursor-pointer"
      >
        ×
      </button>
    </div>

    <p
      style="color: var(--clr-text-secondary);"
      class="text-sm mb-4 truncate"
      title={modName}
    >
      {modName}
    </p>

    <div class="space-y-2 overflow-y-auto mb-5" style="max-height: 320px;">
      {#each files as file (file.fileId)}
        <label
          class="flex items-start gap-3 p-3 rounded cursor-pointer transition-colors"
          style="background: var(--clr-surface-alt, rgba(255,255,255,0.04)); border: 1px solid {selected ===
          file
            ? 'var(--clr-primary-300)'
            : 'var(--adw-border-color)'};"
        >
          <input
            type="radio"
            name="nexus-file"
            bind:group={selected}
            value={file}
            class="mt-1 flex-shrink-0"
          />
          <span class="flex-1 min-w-0">
            <span class="flex items-center gap-2 flex-wrap">
              <span
                class="block text-sm font-medium"
                style="color: var(--clr-text);"
              >
                {file.name ?? file.fileName}
              </span>
              {#if file.version}
                <span
                  class="text-xs px-1.5 py-0.5 rounded"
                  style="color: var(--clr-primary-300); background: color-mix(in srgb, var(--clr-primary-300) 15%, transparent);"
                >
                  v{file.version}
                </span>
              {/if}
            </span>
            {#if file.description}
              <span
                class="block text-xs mt-0.5"
                style="color: var(--clr-text-secondary);"
              >
                {file.description}
              </span>
            {/if}
          </span>
        </label>
      {/each}
    </div>

    <div class="flex gap-2">
      <button on:click={handleCancel} class="flex-1 btn">Cancel</button>
      <button
        on:click={handleDownload}
        disabled={!selected}
        class="flex-1 btn primary"
      >
        Download
      </button>
    </div>
  </div>
</div>
