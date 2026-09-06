<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import type { NexusFileOption } from "$lib/api/commands";
  import ModalShell from "./ModalShell.svelte";

  export let isVisible = true;
  export let modName: string = "";
  export let files: NexusFileOption[] = [];

  const dispatch = createEventDispatcher<{
    select: NexusFileOption[];
    cancel: void;
  }>();

  let selected: NexusFileOption[] = files[0] ? [files[0]] : [];

  function handleDownload() {
    dispatch("select", selected);
  }

  function handleCancel() {
    dispatch("cancel");
  }
</script>

<ModalShell
  {isVisible}
  title="Select file variant"
  titleClass="text-xl font-bold"
  width="w-[520px]"
  headerClass="flex items-center justify-between mb-2"
  closeOnEscape={false}
  on:close={handleCancel}
>
  <p
    style="color: var(--clr-text-secondary);"
    class="text-sm mb-4 truncate"
    title={modName}
  >
    {modName}
  </p>

  <p style="color: var(--clr-text-secondary);" class="text-xs mb-2">
    Select one or more files - mods shipped as multiple parts (e.g. Part 1 +
    Part 2) require all of them.
  </p>

  <div class="space-y-2 overflow-y-auto mb-5" style="max-height: 320px;">
    {#each files as file (file.fileId)}
      <label
        class="flex items-start gap-3 p-3 rounded cursor-pointer transition-colors"
        style="background: var(--clr-surface-alt, rgba(255,255,255,0.04)); border: 1px solid {selected.includes(
          file,
        )
          ? 'var(--clr-primary-300)'
          : 'var(--adw-border-color)'};"
      >
        <input
          type="checkbox"
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
                v{file.version.replace(/^v/i, "")}
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
      disabled={selected.length === 0}
      class="flex-1 btn primary"
    >
      Download
    </button>
  </div>
</ModalShell>
