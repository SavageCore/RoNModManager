<script lang="ts">
  import { createEventDispatcher, tick } from "svelte";
  import { Tag, Trash2 } from "lucide-svelte";

  export let isVisible = false;
  export let modName = "";
  export let modLabel = "";
  export let allTags: string[] = [];
  export let currentTags: string[] = [];

  const dispatch = createEventDispatcher<{
    close: void;
    submit: { modName: string; selectedTags: string[] };
    deleteTag: { tagName: string };
  }>();

  let query = "";
  let selectedTags = new Set<string>();
  let prevIsVisible = false;
  let pendingNewTags: string[] = [];
  let queryInput: HTMLInputElement | null = null;

  // Merge allTags with locally-created (unsaved) tags so they appear in the list immediately
  $: normalizedTags = [...new Set([...allTags, ...pendingNewTags])].sort(
    (a, b) => a.localeCompare(b),
  );

  $: filteredTags = normalizedTags.filter((name) =>
    name.toLowerCase().includes(query.trim().toLowerCase()),
  );

  $: exactMatch = normalizedTags.some(
    (name) => name.toLowerCase() === query.trim().toLowerCase(),
  );

  // Single block so prevIsVisible is checked BEFORE being updated (no Svelte reordering)
  $: {
    if (isVisible && !prevIsVisible) {
      handleOpen();
    } else if (!isVisible && prevIsVisible) {
      handleClose();
    }
    prevIsVisible = isVisible;
  }

  function handleOpen() {
    query = "";
    pendingNewTags = [];
    selectedTags = new Set(currentTags);
    void tick().then(() => {
      queryInput?.focus();
    });
  }

  function handleClose() {
    query = "";
    pendingNewTags = [];
    selectedTags = new Set();
  }

  function closeModal() {
    query = "";
    pendingNewTags = [];
    selectedTags = new Set();
    dispatch("close");
  }

  function toggleTag(name: string) {
    if (selectedTags.has(name)) {
      selectedTags.delete(name);
    } else {
      selectedTags.add(name);
    }
    selectedTags = selectedTags;
  }

  function createAndAdd() {
    const trimmed = query.trim();
    if (!trimmed) return;
    pendingNewTags = [...pendingNewTags, trimmed]; // merges into normalizedTags → appears in list
    selectedTags.add(trimmed);
    selectedTags = selectedTags;
    query = "";
  }

  function requestDeleteTag(tagName: string) {
    // Eagerly remove from local state so the list updates immediately
    selectedTags.delete(tagName);
    selectedTags = selectedTags;
    dispatch("deleteTag", { tagName });
  }

  function submit() {
    dispatch("submit", { modName, selectedTags: Array.from(selectedTags) });
    dispatch("close");
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
          <Tag size={18} />
          Tags
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
        Mod: {modLabel || modName}
      </p>

      <input
        class="input w-full"
        type="text"
        placeholder="Search tags or type new name"
        bind:value={query}
        bind:this={queryInput}
        on:keydown={handleKeydown}
      />

      <div
        style="background: var(--clr-surface-variant); border-color: var(--adw-border-color);"
        class="mt-2 max-h-56 overflow-y-auto rounded border"
      >
        {#if filteredTags.length > 0}
          <ul>
            {#each filteredTags as name (name)}
              {@const isChecked = selectedTags.has(name)}
              <li>
                <div
                  style={isChecked
                    ? "background: color-mix(in srgb, var(--clr-success-300) 15%, transparent);"
                    : ""}
                  class="tag-item flex w-full items-center gap-3 pl-4 pr-5 py-2 text-sm"
                >
                  <label
                    style="color: var(--clr-text);"
                    class="flex flex-1 cursor-pointer items-center gap-3"
                  >
                    <input
                      type="checkbox"
                      checked={isChecked}
                      on:change={() => toggleTag(name)}
                      class="accent-success"
                    />
                    <span class="flex items-center gap-1.5">
                      <Tag size={12} style="color: var(--clr-success-300);" />
                      {name}
                    </span>
                  </label>
                  <button
                    on:click|stopPropagation={() => requestDeleteTag(name)}
                    title="Delete tag"
                    class="delete-btn"
                    aria-label="Delete tag {name}"
                  >
                    <Trash2 size={13} />
                  </button>
                </div>
              </li>
            {/each}
          </ul>
        {:else if query.trim()}
          <div
            class="px-3 py-3 text-sm"
            style="color: var(--clr-text-secondary);"
          >
            No matching tag.
          </div>
        {:else}
          <div
            class="px-3 py-3 text-sm"
            style="color: var(--clr-text-secondary);"
          >
            No tags yet. Type a name to create one.
          </div>
        {/if}
      </div>

      {#if query.trim().length > 0 && !exactMatch}
        <button class="btn primary mt-3" on:click={createAndAdd}>
          Create "{query.trim()}" and select
        </button>
      {/if}

      {#if selectedTags.size > 0}
        <div class="mt-3 flex flex-wrap gap-1">
          {#each Array.from(selectedTags) as tag (tag)}
            <span
              style="background: color-mix(in srgb, var(--clr-success-300) 12%, transparent); border-color: var(--clr-success-300); color: var(--clr-success-300);"
              class="inline-flex items-center gap-1 rounded border px-1.5 py-0.5 text-xs"
            >
              <Tag size={10} />
              {tag}
            </span>
          {/each}
        </div>
      {/if}

      <div class="mt-4 flex justify-end gap-2">
        <button class="btn" on:click={closeModal}>Cancel</button>
        <button class="btn primary" on:click={submit}>Done</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .tag-item:hover {
    background: color-mix(in srgb, var(--clr-text) 8%, transparent);
  }

  .delete-btn {
    opacity: 0;
    transition: opacity 0.15s;
    cursor: pointer;
    color: var(--clr-danger-300);
  }

  .tag-item:hover .delete-btn {
    opacity: 1;
  }
</style>
