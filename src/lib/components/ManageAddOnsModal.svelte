<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import { alertStore } from "$lib/stores/alert";
  import { modAddQueueStore } from "$lib/stores/modAddQueue";
  import { open } from "@tauri-apps/plugin-dialog";
  import type { InstalledModFile } from "$lib/types";

  export let isVisible = false;
  export let modName = "";
  export let displayName = "";
  export let addOns: InstalledModFile[] = [];
  export let noWorldGen = false;

  const dispatch = createEventDispatcher();
  let isDraggingOver = false;

  function closeModal() {
    alertStore.clear();
    dispatch("close");
  }

  async function handleAddOnFile() {
    const selected = await open({
      multiple: true,
      filters: [
        {
          name: "Add-on Files",
          extensions: ["sav", "pak", "zip", "rar", "7z"],
        },
      ],
    });
    if (!selected) return;
    const files = Array.isArray(selected) ? selected : [selected];
    dispatch("addAddOns", { files });
  }

  function handleRemoveAddOn(index: number) {
    dispatch("removeAddOn", { index });
  }

  function handleToggleNoWorldGen() {
    dispatch("toggleNoWorldGen", { exempt: !noWorldGen });
  }
</script>

{#if isVisible}
  <div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
    <div
      class="border rounded-lg shadow-2xl w-[560px] p-6"
      style="background: #23272e; border-color: #444; color: #f3f3f3;"
    >
      <div class="flex items-center justify-between mb-4">
        <h2
          class="text-lg font-semibold"
          style="color: #f3f3f3; background: none;"
        >
          Manage Add-ons for <span style="color: #2196f3;"
            >{displayName || modName}</span
          >
        </h2>
        <button
          on:click={closeModal}
          style="color: #bbb; background: none;"
          class="text-2xl hover:opacity-70 transition cursor-pointer">×</button
        >
      </div>
      <div class="mb-4">
        <button on:click={handleAddOnFile} class="btn primary w-full mb-2"
          >Add Add-on Files</button
        >
        <div class="space-y-2">
          {#each addOns as addOn, i}
            <div
              class="flex items-center justify-between rounded p-2"
              style="background: #2d323b; color: #f3f3f3;"
            >
              <span
                class="truncate min-w-0 flex-1 mr-2 text-sm"
                title={addOn.name}>{addOn.name}</span
              >
              <button
                on:click={() => handleRemoveAddOn(i)}
                class="btn danger btn-xs flex-shrink-0">Remove</button
              >
            </div>
          {/each}
          {#if addOns.length === 0}
            <div class="text-sm" style="color: #aaa;">
              No add-ons added yet.
            </div>
          {/if}
        </div>
      </div>
      <div
        class="flex items-center justify-between rounded p-3 mt-4"
        style="background: #2d323b;"
      >
        <div>
          <div class="text-sm font-medium" style="color: #f3f3f3;">
            No world generation required
          </div>
          <div class="text-xs mt-0.5" style="color: #aaa;">
            Suppresses the missing world generation warning for this map.
          </div>
        </div>
        <button
          role="switch"
          aria-checked={noWorldGen}
          aria-label="No world generation required"
          on:click={handleToggleNoWorldGen}
          class="relative inline-flex h-5 w-9 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200"
          style="background: {noWorldGen ? '#2196f3' : '#555'};"
        >
          <span
            class="pointer-events-none inline-block h-4 w-4 rounded-full shadow transition-transform duration-200"
            style="background: #fff; transform: translateX({noWorldGen
              ? '16px'
              : '0px'});"
          ></span>
        </button>
      </div>
      <div class="flex gap-2 mt-4">
        <button on:click={closeModal} class="flex-1 btn">Close</button>
      </div>
    </div>
  </div>
{/if}
