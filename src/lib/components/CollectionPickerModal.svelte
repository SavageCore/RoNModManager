<script lang="ts">
  import { tick } from "svelte";
  import { createEventDispatcher } from "svelte";

  export let isVisible = false;
  export let modName = "";
  export let collections: string[] = [];

  const dispatch = createEventDispatcher<{
    close: void;
    submit: { collectionName: string };
  }>();

  let query = "";
  let selectedIndex = 0;
  let wasVisible = false;
  let queryInput: HTMLInputElement | null = null;

  $: normalizedCollections = [...collections].sort((a, b) =>
    a.localeCompare(b),
  );

  $: filteredCollections = normalizedCollections.filter((name) =>
    name.toLowerCase().includes(query.trim().toLowerCase()),
  );

  $: exactMatch = normalizedCollections.some(
    (name) => name.toLowerCase() === query.trim().toLowerCase(),
  );

  $: if (selectedIndex >= filteredCollections.length) {
    selectedIndex =
      filteredCollections.length > 0 ? filteredCollections.length - 1 : 0;
  }

  $: if (isVisible && !wasVisible) {
    query = "";
    selectedIndex = 0;
    void tick().then(() => {
      queryInput?.focus();
    });
  }

  $: if (!isVisible && wasVisible) {
    query = "";
    selectedIndex = 0;
  }

  $: wasVisible = isVisible;

  function closeModal() {
    query = "";
    selectedIndex = 0;
    dispatch("close");
  }

  function submit(name: string) {
    const trimmed = name.trim();
    if (!trimmed) {
      return;
    }
    query = "";
    selectedIndex = 0;
    dispatch("submit", { collectionName: trimmed });
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      closeModal();
      return;
    }

    if (event.key === "ArrowDown") {
      event.preventDefault();
      if (filteredCollections.length > 0) {
        selectedIndex = Math.min(
          selectedIndex + 1,
          filteredCollections.length - 1,
        );
      }
      return;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      if (filteredCollections.length > 0) {
        selectedIndex = Math.max(selectedIndex - 1, 0);
      }
      return;
    }

    if (event.key === "Enter") {
      event.preventDefault();
      if (filteredCollections.length > 0) {
        submit(filteredCollections[selectedIndex]);
      } else {
        submit(query);
      }
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
        <h2 style="color: var(--clr-text);" class="text-lg font-semibold">
          Add To Collection
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
        Mod: {modName}
      </p>

      <input
        class="input w-full"
        type="text"
        placeholder="Search collections or type new name"
        bind:value={query}
        bind:this={queryInput}
        on:keydown={handleKeydown}
      />

      <div
        style="background: var(--clr-surface-variant); border-color: var(--adw-border-color);"
        class="mt-2 max-h-56 overflow-y-auto rounded border"
      >
        {#if filteredCollections.length > 0}
          <ul>
            {#each filteredCollections as name, index (name)}
              <li>
                <button
                  style={index === selectedIndex
                    ? "background: color-mix(in srgb, var(--clr-primary-300) 18%, transparent); color: var(--clr-text);"
                    : "color: var(--clr-text);"}
                  class="w-full cursor-pointer px-3 py-2 text-left text-sm hover:opacity-90"
                  on:click={() => submit(name)}
                >
                  {name}
                </button>
              </li>
            {/each}
          </ul>
        {:else}
          <div
            class="px-3 py-3 text-sm"
            style="color: var(--clr-text-secondary);"
          >
            No matching collection.
          </div>
        {/if}
      </div>

      {#if query.trim().length > 0 && !exactMatch}
        <button class="btn primary mt-3" on:click={() => submit(query)}>
          Create "{query.trim()}" and add mod
        </button>
      {/if}

      <div class="mt-4 flex justify-end gap-2">
        <button class="btn" on:click={closeModal}>Cancel</button>
      </div>
    </div>
  </div>
{/if}
