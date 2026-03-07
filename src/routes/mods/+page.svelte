<script lang="ts">
  import { onMount } from "svelte";
  import { getModList, uninstallMods, installMods } from "$lib/api/commands";
  import ProgressModal from "$lib/components/ProgressModal.svelte";
  import { listen } from "@tauri-apps/api/event";
  import type { ModInfo, ProgressEvent } from "$lib/types";

  let mods: ModInfo[] = [];
  let message = "";
  let isInstallingMods = false;
  let showProgress = false;
  let currentProgress: ProgressEvent | null = null;

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

  async function installModsHandler() {
    showProgress = true;
    isInstallingMods = true;
    message = "";

    try {
      await installMods();
      message = "Mods installed successfully!";
      await refresh();
    } catch (error) {
      message = `Installation failed: ${String(error)}`;
    } finally {
      isInstallingMods = false;
    }
  }

  onMount(() => {
    void refresh();

    // Listen for progress events from the backend
    const unsubscribe = listen<ProgressEvent>("install_progress", (event) => {
      currentProgress = event.payload;
    });

    return unsubscribe;
  });
</script>

<ProgressModal
  isVisible={showProgress}
  progress={currentProgress}
  onCancel={() => {
    showProgress = false;
    isInstallingMods = false;
  }}
/>

<section
  class="rounded-2xl border border-teal-700/15 bg-[var(--color-surface)] p-6 shadow-sm"
>
  <div class="flex items-center justify-between">
    <h1 class="text-2xl font-semibold">Mods</h1>
    <div class="flex gap-2">
      <button
        class="rounded-lg bg-teal-600 px-3 py-2 text-sm text-white hover:bg-teal-700 transition disabled:opacity-50 disabled:cursor-not-allowed"
        disabled={isInstallingMods}
        on:click={installModsHandler}>Install Mods</button
      >
      <button
        class="rounded-lg bg-zinc-900 px-3 py-2 text-sm text-white hover:bg-zinc-800 transition disabled:opacity-50 disabled:cursor-not-allowed"
        disabled={isInstallingMods}
        on:click={uninstallAll}>Uninstall All</button
      >
    </div>
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
