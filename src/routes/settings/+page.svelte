<script lang="ts">
  import { onMount } from "svelte";
  import {
    detectGamePath,
    getAuthStatus,
    getConfig,
    logout,
    openModioLogin,
    saveToken,
    setGamePath,
    setModpackUrl,
    setTheme,
    validateToken,
  } from "$lib/api/commands";

  let gamePath = "";
  let modpackUrl = "";
  let token = "";
  let authConnected = false;
  let theme: "light" | "dark" | "system" = "system";
  let message = "";

  async function refresh() {
    const config = await getConfig();
    authConnected = await getAuthStatus().catch(() => false);
    gamePath = config.game_path ?? "";
    modpackUrl = config.modpack_url ?? "";
    theme = config.theme;
  }

  async function autodetect() {
    const detected = await detectGamePath();
    if (!detected) {
      message = "Could not auto-detect game path.";
      return;
    }
    gamePath = detected;
    await setGamePath(detected);
    message = "Auto-detected game path.";
  }

  async function save() {
    await setGamePath(gamePath.trim());
    await setModpackUrl(modpackUrl.trim());
    await setTheme(theme);
    message = "Settings saved.";
    await refresh();
  }

  async function saveManualToken() {
    await saveToken(token.trim());
    token = "";
    message = "Token saved.";
    await refresh();
  }

  async function checkToken() {
    const valid = await validateToken();
    message = valid ? "Token is valid." : "Token is invalid/expired.";
  }

  async function disconnect() {
    await logout();
    message = "Logged out.";
    await refresh();
  }

  onMount(() => {
    void refresh();
  });
</script>

<section
  class="rounded-2xl border border-teal-700/15 bg-[var(--color-surface)] p-6 shadow-sm"
>
  <h1 class="text-2xl font-semibold">Settings</h1>

  {#if message}
    <p class="mt-4 rounded-lg bg-teal-50 px-3 py-2 text-sm text-teal-700">
      {message}
    </p>
  {/if}

  <div class="mt-4 grid gap-4 md:grid-cols-2">
    <label class="block text-sm">
      <span class="mb-1 block text-[var(--color-muted)]">Game Path</span>
      <input
        class="w-full rounded-lg border border-zinc-300 px-3 py-2"
        bind:value={gamePath}
      />
      <button
        class="mt-2 rounded-lg bg-zinc-900 px-3 py-2 text-xs text-white"
        on:click={autodetect}>Auto Detect</button
      >
    </label>

    <label class="block text-sm">
      <span class="mb-1 block text-[var(--color-muted)]">Modpack URL</span>
      <input
        class="w-full rounded-lg border border-zinc-300 px-3 py-2"
        bind:value={modpackUrl}
      />
    </label>

    <label class="block text-sm">
      <span class="mb-1 block text-[var(--color-muted)]">Theme</span>
      <select
        class="w-full rounded-lg border border-zinc-300 px-3 py-2"
        bind:value={theme}
      >
        <option value="system">System</option>
        <option value="light">Light</option>
        <option value="dark">Dark</option>
      </select>
    </label>

    <div class="text-sm">
      <p class="text-[var(--color-muted)]">
        mod.io status: {authConnected ? "connected" : "not connected"}
      </p>
      <div class="mt-2 flex flex-wrap gap-2">
        <button
          class="rounded-lg bg-zinc-900 px-3 py-2 text-xs text-white"
          on:click={openModioLogin}>Open Login</button
        >
        <button
          class="rounded-lg bg-zinc-700 px-3 py-2 text-xs text-white"
          on:click={checkToken}>Validate</button
        >
        <button
          class="rounded-lg bg-zinc-500 px-3 py-2 text-xs text-white"
          on:click={disconnect}>Logout</button
        >
      </div>
    </div>
  </div>

  <label class="mt-4 block text-sm">
    <span class="mb-1 block text-[var(--color-muted)]">Manual OAuth Token</span>
    <input
      class="w-full rounded-lg border border-zinc-300 px-3 py-2"
      bind:value={token}
    />
    <button
      class="mt-2 rounded-lg bg-teal-700 px-3 py-2 text-xs text-white"
      on:click={saveManualToken}>Save Token</button
    >
  </label>

  <button
    class="mt-5 rounded-lg bg-teal-800 px-4 py-2 text-sm text-white"
    on:click={save}>Save All Settings</button
  >
</section>
