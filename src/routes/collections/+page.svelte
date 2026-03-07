<script lang="ts">
  import { onMount } from "svelte";
  import { getCollections, toggleCollection } from "$lib/api/commands";

  let collections: Record<string, boolean> = {};
  let message = "";

  async function refresh() {
    try {
      collections = await getCollections();
    } catch (error) {
      message = `Failed to load collections: ${String(error)}`;
    }
  }

  async function onToggle(name: string, enabled: boolean) {
    await toggleCollection(name, enabled);
    collections[name] = enabled;
    message = `${name} ${enabled ? "enabled" : "disabled"}.`;
  }

  onMount(() => {
    void refresh();
  });
</script>

<section
  class="rounded-2xl border border-teal-700/15 bg-[var(--color-surface)] p-6 shadow-sm"
>
  <h1 class="text-2xl font-semibold">Collections</h1>

  {#if message}
    <p class="mt-4 rounded-lg bg-teal-50 px-3 py-2 text-sm text-teal-700">
      {message}
    </p>
  {/if}

  {#if Object.keys(collections).length === 0}
    <p class="mt-4 text-sm text-[var(--color-muted)]">
      No collections configured yet.
    </p>
  {:else}
    <ul class="mt-4 space-y-2">
      {#each Object.entries(collections) as [name, enabled] (name)}
        <li
          class="flex items-center justify-between rounded-lg border border-zinc-200 px-3 py-2 text-sm"
        >
          <span>{name}</span>
          <input
            type="checkbox"
            checked={enabled}
            on:change={(event) =>
              onToggle(name, (event.currentTarget as HTMLInputElement).checked)}
          />
        </li>
      {/each}
    </ul>
  {/if}
</section>
