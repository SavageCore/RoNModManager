<script lang="ts">
  import { onMount } from "svelte";
  import { getModList, uninstallMods } from "$lib/api/commands";
  import type { ModInfo } from "$lib/types";

  let mods: ModInfo[] = [];
  let message = "";

  async function refresh() {
    try {
      mods = await getModList();
    } catch (error) {
      message = `Failed to load mods: ${String(error)}`;
    }
  }

  async function uninstallAll() {
    await uninstallMods();
    message = "Uninstalled all .pak files from active ~mods folder.";
    await refresh();
  }

  onMount(() => {
    void refresh();
  });
</script>

<section
  class="rounded-2xl border border-teal-700/15 bg-[var(--color-surface)] p-6 shadow-sm"
>
  <div class="flex items-center justify-between">
    <h1 class="text-2xl font-semibold">Mods</h1>
    <button
      class="rounded-lg bg-zinc-900 px-3 py-2 text-sm text-white"
      on:click={uninstallAll}>Uninstall All</button
    >
  </div>

  {#if message}
    <p class="mt-4 rounded-lg bg-teal-50 px-3 py-2 text-sm text-teal-700">
      {message}
    </p>
  {/if}

  {#if mods.length === 0}
    <p class="mt-4 text-sm text-[var(--color-muted)]">
      No installed .pak mods found.
    </p>
  {:else}
    <ul class="mt-4 space-y-2">
      {#each mods as mod (mod.filename)}
        <li class="rounded-lg border border-zinc-200 px-3 py-2 text-sm">
          {mod.filename}
        </li>
      {/each}
    </ul>
  {/if}
</section>
