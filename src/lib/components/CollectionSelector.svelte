<script lang="ts">
  import type { Collection } from "$lib/types";

  export let isVisible = false;
  export let collections: Record<string, Collection> = {};
  export let selectedCollections: Set<string> = new Set();
  export let onConfirm: (selected: string[]) => void = () => {};
  export let isLoading = false;

  let localSelected: string[] = Array.from(selectedCollections);

  function toggle(name: string) {
    const index = localSelected.indexOf(name);
    if (index > -1) {
      localSelected.splice(index, 1);
    } else {
      localSelected.push(name);
    }
    localSelected = localSelected; // Trigger reactivity
  }

  function confirm() {
    onConfirm(localSelected);
    isVisible = false;
  }

  function cancel() {
    localSelected = Array.from(selectedCollections);
    isVisible = false;
  }

  $: if (isVisible) {
    localSelected = Array.from(selectedCollections);
  }
</script>

{#if isVisible}
  <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
    <div
      class="bg-white dark:bg-slate-800 rounded-lg shadow-2xl w-96 max-h-96 overflow-auto p-6"
    >
      <h2 class="text-2xl font-bold mb-4">Select Collections</h2>

      {#if isLoading}
        <p class="text-center text-slate-500">Loading collections...</p>
      {:else if Object.keys(collections).length === 0}
        <p class="text-center text-slate-500">No collections available</p>
      {:else}
        <div class="space-y-2 mb-4">
          {#each Object.entries(collections) as [name, collection] (name)}
            <label class="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={localSelected.includes(name)}
                on:change={() => toggle(name)}
                class="rounded"
              />
              <div class="flex-1">
                <div class="font-medium text-sm">{name}</div>
                <div class="text-xs text-slate-500">
                  {collection.description}
                </div>
              </div>
            </label>
          {/each}
        </div>
      {/if}

      <div class="flex gap-2 justify-end">
        <button
          class="rounded-lg bg-slate-500 px-3 py-2 text-sm text-white"
          on:click={cancel}
          disabled={isLoading}
        >
          Cancel
        </button>
        <button
          class="rounded-lg bg-teal-700 px-3 py-2 text-sm text-white disabled:opacity-50"
          on:click={confirm}
          disabled={isLoading}
        >
          Install
        </button>
      </div>
    </div>
  </div>
{/if}
