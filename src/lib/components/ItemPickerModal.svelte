<script lang="ts">
  import { createEventDispatcher, tick } from "svelte";
  import { Trash2 } from "lucide-svelte";

  export let isVisible = false;
  export let modLabel = "";
  export let allItems: string[] = [];
  export let currentItems: string[] = [];
  export let title: string;
  export let ItemIcon: any;
  export let accentColorVar: string;
  export let searchPlaceholder: string;
  export let createButtonText: (name: string) => string;
  export let allowDelete = false;
  export let noteText = "";
  export let partialItems: string[] = [];
  export let subtitle = "";

  const dispatch = createEventDispatcher<{
    close: void;
    toggle: { itemName: string };
    create: { itemName: string };
    deleteItem: { itemName: string };
  }>();

  let query = "";
  let pendingNewItems: string[] = [];
  let wasVisible = false;
  let queryInput: HTMLInputElement | null = null;

  $: normalizedItems = [...new Set([...allItems, ...pendingNewItems])].sort(
    (a, b) => a.localeCompare(b),
  );

  $: filteredItems = normalizedItems.filter((name) =>
    name.toLowerCase().includes(query.trim().toLowerCase()),
  );

  $: exactMatch = normalizedItems.some(
    (name) => name.toLowerCase() === query.trim().toLowerCase(),
  );

  // Remove pending items once the backend confirms them in allItems
  $: pendingNewItems = pendingNewItems.filter(
    (name) => !allItems.includes(name),
  );

  $: if (isVisible && !wasVisible) {
    query = "";
    pendingNewItems = [];
    void tick().then(() => {
      queryInput?.focus();
    });
  }

  $: if (!isVisible && wasVisible) {
    query = "";
    pendingNewItems = [];
  }

  $: wasVisible = isVisible;

  function closeModal() {
    query = "";
    pendingNewItems = [];
    dispatch("close");
  }

  function toggleItem(name: string) {
    dispatch("toggle", { itemName: name });
  }

  function createAndAdd() {
    const trimmed = query.trim();
    if (!trimmed) return;
    pendingNewItems = [...pendingNewItems, trimmed];
    query = "";
    dispatch("create", { itemName: trimmed });
  }

  function requestDelete(name: string) {
    dispatch("deleteItem", { itemName: name });
  }

  function setIndeterminate(node: HTMLInputElement, value: boolean) {
    node.indeterminate = value;
    return {
      update(v: boolean) {
        node.indeterminate = v;
      },
    };
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      closeModal();
    }
    if (event.key === "Enter" && query.trim() && !exactMatch) {
      event.preventDefault();
      createAndAdd();
    }
  }
</script>

{#if isVisible}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
    <div
      style="background: var(--clr-surface); border-color: var(--adw-border-color);"
      class="w-[540px] max-w-[92vw] rounded-lg border p-5 shadow-2xl"
    >
      <div class="mb-3 flex items-center justify-between gap-3">
        <h2
          style="color: var(--clr-text);"
          class="text-lg font-semibold flex items-center gap-2"
        >
          <svelte:component this={ItemIcon} size={18} />
          {title}
        </h2>
        <button
          on:click={closeModal}
          style="color: var(--clr-text-secondary);"
          class="cursor-pointer text-xl leading-none hover:opacity-70"
          aria-label="Close"
        >
          ×
        </button>
      </div>

      <p style="color: var(--clr-text-secondary);" class="mb-3 text-sm">
        {subtitle || `Mod: ${modLabel}`}
      </p>

      <input
        class="input w-full"
        type="text"
        placeholder={searchPlaceholder}
        bind:value={query}
        bind:this={queryInput}
        on:keydown={handleKeydown}
      />

      <div
        style="background: var(--clr-surface-variant); border-color: var(--adw-border-color);"
        class="mt-2 max-h-56 overflow-y-auto rounded border"
      >
        {#if filteredItems.length > 0}
          <ul>
            {#each filteredItems as name (name)}
              {@const isChecked =
                (currentItems.includes(name) ||
                  pendingNewItems.includes(name)) &&
                !partialItems.includes(name)}
              {@const isPartial =
                partialItems.includes(name) && !currentItems.includes(name)}
              <li>
                <div
                  style={isChecked || isPartial
                    ? `background: color-mix(in srgb, var(${accentColorVar}) 15%, transparent);`
                    : ""}
                  class="item-row flex w-full items-center gap-3 pl-4 pr-5 py-2 text-sm"
                >
                  <label
                    style="color: var(--clr-text);"
                    class="flex flex-1 cursor-pointer items-center gap-3"
                  >
                    <input
                      type="checkbox"
                      checked={isChecked}
                      use:setIndeterminate={isPartial}
                      on:change={() => toggleItem(name)}
                      style={`accent-color: var(${accentColorVar});`}
                    />
                    <span class="flex items-center gap-1.5">
                      <svelte:component
                        this={ItemIcon}
                        size={12}
                        style={`color: var(${accentColorVar});`}
                      />
                      {name}
                    </span>
                  </label>
                  {#if allowDelete}
                    <button
                      on:click|stopPropagation={() => requestDelete(name)}
                      title="Delete {title.toLowerCase().replace(/s$/, '')}"
                      class="delete-btn"
                      aria-label="Delete {name}"
                    >
                      <Trash2 size={13} />
                    </button>
                  {/if}
                </div>
              </li>
            {/each}
          </ul>
        {:else if query.trim()}
          <div
            class="px-3 py-3 text-sm"
            style="color: var(--clr-text-secondary);"
          >
            No matching {title.toLowerCase().replace(/s$/, "")}.
          </div>
        {:else}
          <div
            class="px-3 py-3 text-sm"
            style="color: var(--clr-text-secondary);"
          >
            No {title.toLowerCase()} yet. Type a name to create one.
          </div>
        {/if}
      </div>

      {#if query.trim().length > 0 && !exactMatch}
        <button class="btn primary mt-3" on:click={createAndAdd}>
          {createButtonText(query.trim())}
        </button>
      {/if}

      {#if currentItems.length > 0 || pendingNewItems.length > 0}
        <div class="mt-3 flex flex-wrap gap-1">
          {#each [...new Set( [...currentItems, ...pendingNewItems], )] as item (item)}
            <span
              style={`background: color-mix(in srgb, var(${accentColorVar}) 12%, transparent); border-color: var(${accentColorVar}); color: var(${accentColorVar});`}
              class="inline-flex items-center gap-1 rounded border px-1.5 py-0.5 text-xs"
            >
              <svelte:component this={ItemIcon} size={10} />
              {item}
            </span>
          {/each}
        </div>
      {/if}

      {#if noteText}
        <p class="mt-2 text-xs" style="color: var(--clr-text-secondary);">
          {noteText}
        </p>
      {/if}

      <div class="mt-4 flex justify-end gap-2">
        <button class="btn" on:click={closeModal}>Done</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .item-row:hover {
    background: color-mix(in srgb, var(--clr-text) 8%, transparent);
  }

  .delete-btn {
    opacity: 0;
    transition: opacity 0.15s;
    cursor: pointer;
    color: var(--clr-danger-300);
  }

  .item-row:hover .delete-btn {
    opacity: 1;
  }
</style>
