<script lang="ts">
  import { onMount } from "svelte";
  import {
    detectGamePath,
    getConfig,
    getModList,
    setGamePath,
    setModpackUrl,
    syncModpack,
    uninstallMods,
  } from "$lib/api/commands";
  import { toastStore } from "$lib/stores/toast";

  let modpackUrl = "";
  let gamePath = "";
  let installedCount = 0;
  let isConfigured = false;
  let requiresManualSetup = false;

  async function refresh() {
    try {
      const [config, mods] = await Promise.all([
        getConfig(),
        getModList().catch(() => []),
      ]);
      modpackUrl = config.modpack_url ?? "";
      gamePath = config.game_path ?? "";
      installedCount = mods.length;
      isConfigured = config.game_path != null;
    } catch (error) {
      toastStore.error(`Failed loading app state: ${String(error)}`);
    }
  }

  async function autodetectIfNeeded() {
    if (isConfigured) {
      requiresManualSetup = false;
      return;
    }

    try {
      const detected = await detectGamePath();
      if (!detected) {
        requiresManualSetup = true;
        return;
      }

      await setGamePath(detected);
      toastStore.success("Game path auto-detected and saved.");
      requiresManualSetup = false;
      await refresh();
    } catch (error) {
      requiresManualSetup = true;
      toastStore.error(`Auto-detect failed: ${String(error)}`);
    }
  }

  async function save() {
    await setGamePath(gamePath.trim());
    await setModpackUrl(modpackUrl.trim());
    toastStore.success("Settings saved.");
    await refresh();
  }

  async function sync() {
    const modpack = await syncModpack();
    toastStore.success(`Synced ${modpack.name} v${modpack.version}.`);
    await refresh();
  }

  async function clearInstalled() {
    await uninstallMods();
    toastStore.success(
      "Removed installed .pak mods from active profile folder.",
    );
    await refresh();
  }

  onMount(async () => {
    await refresh();
    await autodetectIfNeeded();
  });
</script>

<main
  class="mx-auto flex min-h-screen w-full max-w-4xl flex-col gap-6 px-6 py-10 sm:px-10"
>
  <header
    class="rounded-2xl border border-teal-700/15 bg-[var(--color-surface)] p-6 shadow-sm"
  >
    <p class="text-sm font-semibold uppercase tracking-[0.2em] text-teal-700">
      RoN Mod Manager
    </p>
    <h1 class="mt-2 text-3xl font-semibold">Dashboard</h1>
    <p class="mt-3 text-[var(--color-muted)]">
      Manage your modpack and installed mods.
    </p>
    {#if !isConfigured && requiresManualSetup}
      <div
        class="mt-4 flex items-center gap-3 rounded-lg border border-yellow-200 bg-yellow-50 px-4 py-3"
      >
        <div class="flex-1">
          <p class="text-sm font-medium text-yellow-900">
            Game path was not found. Open Settings to set it manually before
            continuing.
          </p>
        </div>
        <button
          class="whitespace-nowrap rounded-lg bg-yellow-600 px-3 py-1 text-sm font-medium text-white hover:bg-yellow-700"
          on:click={() => {
            window.location.href = "/settings";
          }}
        >
          Go to Settings
        </button>
      </div>
    {/if}
  </header>

  {#if isConfigured}
    <section class="grid gap-4 sm:grid-cols-2">
      <article
        class="rounded-2xl border border-teal-700/15 bg-[var(--color-surface)] p-5"
      >
        <h2 class="text-lg font-semibold">Game Path</h2>
        <p class="mt-2 text-sm text-[var(--color-muted)]">{gamePath}</p>
      </article>

      <article
        class="rounded-2xl border border-teal-700/15 bg-[var(--color-surface)] p-5"
      >
        <h2 class="text-lg font-semibold">Modpack</h2>
        <input
          class="mt-3 w-full rounded-lg border border-zinc-300 px-3 py-2"
          bind:value={modpackUrl}
        />
        <div class="mt-3 flex gap-2">
          <button
            class="rounded-lg bg-zinc-900 px-3 py-2 text-sm text-white"
            on:click={save}>Save URL</button
          >
          <button
            class="rounded-lg bg-teal-700 px-3 py-2 text-sm text-white"
            on:click={sync}>Sync</button
          >
        </div>
      </article>
    </section>

    <section
      class="rounded-2xl border border-teal-700/15 bg-[var(--color-surface)] p-5"
    >
      <h2 class="text-lg font-semibold">Installed Mods</h2>
      <p class="mt-2 text-[var(--color-muted)]">
        Detected .pak files in `~mods`: <strong>{installedCount}</strong>
      </p>
      <button
        class="mt-3 rounded-lg bg-zinc-800 px-3 py-2 text-sm text-white"
        on:click={clearInstalled}
      >
        Uninstall All
      </button>
    </section>
  {/if}
</main>
