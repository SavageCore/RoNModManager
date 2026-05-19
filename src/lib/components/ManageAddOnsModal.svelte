<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import { alertStore } from "$lib/stores/alert";
  import { modAddQueueStore } from "$lib/stores/modAddQueue";
  import { open } from "@tauri-apps/plugin-dialog";

  export let isVisible = false;
  export let modName = "";
  export let addOns = [];

  const dispatch = createEventDispatcher();
  let isDraggingOver = false;

  function closeModal() {
    alertStore.clear();
    dispatch("close");
  }

  async function handleAddOnFile() {
    const selected = await open({
      multiple: true,
      filters: [{ name: "Add-on Files", extensions: ["sav", "pak", "zip", "rar", "7z"] }],
    });
    if (!selected) return;
    const files = Array.isArray(selected) ? selected : [selected];
    dispatch("addAddOns", { files });
  }

  function handleRemoveAddOn(index: number) {
    dispatch("removeAddOn", { index });
  }
</script>

{#if isVisible}
  <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
    <div class="border rounded-lg shadow-2xl w-[560px] p-6" style="background: var(--clr-surface); border-color: var(--adw-border-color);">
      <div class="flex items-center justify-between mb-4">
        <h2 style="color: var(--clr-text);" class="text-2xl font-bold">
          Manage Add-ons for {modName}
        </h2>
        <button on:click={closeModal} style="color: var(--clr-text-secondary);" class="text-2xl hover:opacity-70 transition cursor-pointer">×</button>
      </div>
      <div class="mb-4">
        <button on:click={handleAddOnFile} class="btn primary w-full mb-2">Add Add-on Files</button>
        <div class="space-y-2">
          {#each addOns as addOn, i}
            <div class="flex items-center justify-between bg-gray-100 rounded p-2">
              <span>{addOn.name}</span>
              <button on:click={() => handleRemoveAddOn(i)} class="btn danger btn-xs">Remove</button>
            </div>
          {/each}
          {#if addOns.length === 0}
            <div class="text-sm text-gray-500">No add-ons added yet.</div>
          {/if}
        </div>
      </div>
      <div class="flex gap-2 mt-4">
        <button on:click={closeModal} class="flex-1 btn">Close</button>
      </div>
    </div>
  </div>
{/if}
