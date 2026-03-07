<script lang="ts">
  import { onMount } from "svelte";
  import { getCollections, toggleCollection } from "$lib/api/commands";
  import { toastStore } from "$lib/stores/toast";

  let collections: Record<string, boolean> = {};

  async function refresh() {
    try {
      collections = await getCollections();
    } catch (error) {
      toastStore.error(`Failed to load collections: ${String(error)}`);
    }
  }

  async function onToggle(name: string, enabled: boolean) {
    await toggleCollection(name, enabled);
    collections[name] = enabled;
    toastStore.success(`${name} ${enabled ? "enabled" : "disabled"}.`);
  }

  onMount(() => {
    void refresh();
  });
</script>

<section class="card">
  <h1 style="color: var(--clr-text);" class="mb-4 text-2xl font-semibold">
    Collections
  </h1>

  {#if Object.keys(collections).length === 0}
    <p style="color: var(--clr-text-secondary);" class="mt-4 text-sm">
      No collections configured yet.
    </p>
  {:else}
    <ul class="mt-4 space-y-2">
      {#each Object.entries(collections) as [name, enabled] (name)}
        <li
          style="background: var(--clr-surface); border-color: var(--adw-border-color); color: var(--clr-text);"
          class="flex items-center justify-between rounded-lg border px-3 py-2 text-sm"
        >
          <span>{name}</span>
          <input
            type="checkbox"
            class="checkbox"
            checked={enabled}
            on:change={(event) =>
              onToggle(name, (event.currentTarget as HTMLInputElement).checked)}
          />
        </li>
      {/each}
    </ul>
  {/if}
</section>
