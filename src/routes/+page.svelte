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

  let message = "";
  let modpackUrl = "";
  let gamePath = "";
  let installedCount = 0;

  async function refresh() {
    try {
      const [config, mods] = await Promise.all([
        getConfig(),
        getModList().catch(() => []),
      ]);
      modpackUrl = config.modpack_url ?? "";
      gamePath = config.game_path ?? "";
      installedCount = mods.length;
    } catch (error) {
      message = `Failed loading app state: ${String(error)}`;
    }
  }

  async function autodetect() {
    const detected = await detectGamePath();
    if (!detected) {
      message = "Unable to auto-detect game path.";
      return;
    }
    gamePath = detected;
    await setGamePath(detected);
    message = "Game path detected and saved.";
    await refresh();
  }

  async function save() {
    await setGamePath(gamePath.trim());
    await setModpackUrl(modpackUrl.trim());
    message = "Settings saved.";
    await refresh();
  }

  async function sync() {
    const modpack = await syncModpack();
    message = `Synced ${modpack.name} v${modpack.version}.`;
    await refresh();
  }

  async function clearInstalled() {
    await uninstallMods();
    message = "Removed installed .pak mods from active profile folder.";
    await refresh();
  }

  onMount(() => {
    void refresh();
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
    <h1 class="mt-2 text-3xl font-semibold">Backend Connected</h1>
    <p class="mt-3 text-[var(--color-muted)]">
      IPC commands are live and this page exercises core flows.
    </p>
    {#if message}
      <p class="mt-3 rounded-lg bg-teal-50 px-3 py-2 text-sm text-teal-700">
        {message}
      </p>
    {/if}
  </header>

  <section class="grid gap-4 sm:grid-cols-2">
    <article
      class="rounded-2xl border border-teal-700/15 bg-[var(--color-surface)] p-5"
    >
      <h2 class="text-lg font-semibold">Game Path</h2>
      <input
        class="mt-3 w-full rounded-lg border border-zinc-300 px-3 py-2"
        bind:value={gamePath}
      />
      <div class="mt-3 flex gap-2">
        <button
          class="rounded-lg bg-zinc-900 px-3 py-2 text-sm text-white"
          on:click={autodetect}>Auto Detect</button
        >
        <button
          class="rounded-lg bg-teal-700 px-3 py-2 text-sm text-white"
          on:click={save}>Save</button
        >
      </div>
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
          class="rounded-lg bg-teal-900 px-3 py-2 text-sm text-white"
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
</main>
