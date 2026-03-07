<script lang="ts">
  import { onMount } from "svelte";
  import {
    applyIntroSkip,
    buildModpackFromInstalled,
    checkForUpdate,
    detectGamePath,
    installUpdate,
    exportModpackToFile,
    getAuthStatus,
    getConfig,
    isIntroSkipApplied,
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
  let introSkipApplied = false;
  let applyingIntroSkip = false;
  let updateCheckInProgress = false;
  let updateInstallInProgress = false;
  let updateVersion: string | null = null;

  async function refresh() {
    const config = await getConfig();
    authConnected = await getAuthStatus().catch(() => false);
    introSkipApplied = await isIntroSkipApplied().catch(() => false);
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

  async function applyIntroSkipConfig() {
    applyingIntroSkip = true;
    try {
      await applyIntroSkip();
      introSkipApplied = true;
      message = "Intro skip applied successfully!";
    } catch (error) {
      message = `Failed to apply intro skip: ${error}`;
    } finally {
      applyingIntroSkip = false;
    }
  }

  async function exportInstalledMods() {
    try {
      const modpack = await buildModpackFromInstalled();
      const timestamp = new Date()
        .toISOString()
        .replace(/[:.]/g, "-")
        .split("T")[0];
      // Save to app config directory with timestamp
      const filename = `ronmod-export-${timestamp}.json`;
      await exportModpackToFile(modpack, `./modpacks/${filename}`);
      message = `Modpack exported successfully! Filename: ${filename}`;
    } catch (error) {
      message = `Failed to export modpack: ${error}`;
    }
  }

  async function checkUpdates() {
    updateCheckInProgress = true;
    try {
      const info = await checkForUpdate();
      if (info.available) {
        updateVersion = info.version;
        message = `Update available: ${info.version}`;
      } else {
        updateVersion = null;
        message = "No updates available.";
      }
    } catch (error) {
      message = `Failed to check updates: ${error}`;
    } finally {
      updateCheckInProgress = false;
    }
  }

  async function installAvailableUpdate() {
    updateInstallInProgress = true;
    try {
      const info = await installUpdate();
      if (info.available) {
        message =
          "Update installed. The app may close automatically on Windows; otherwise restart to use the new version.";
      } else {
        message = "No update available to install.";
      }
    } catch (error) {
      message = `Failed to install update: ${error}`;
    } finally {
      updateInstallInProgress = false;
    }
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

  <div class="mt-4 rounded-lg border border-zinc-200 bg-zinc-50 p-4">
    <div class="flex items-center justify-between">
      <div>
        <h3 class="font-semibold text-zinc-900">Intro Skip</h3>
        <p class="text-sm text-[var(--color-muted)]">
          {introSkipApplied
            ? "✓ Applied"
            : "Configure Game.ini to skip intro video"}
        </p>
      </div>
      <button
        class="rounded-lg bg-teal-700 px-3 py-2 text-xs text-white disabled:opacity-50"
        disabled={applyingIntroSkip || introSkipApplied}
        on:click={applyIntroSkipConfig}
      >
        {applyingIntroSkip
          ? "Applying..."
          : introSkipApplied
            ? "Applied"
            : "Apply"}
      </button>
    </div>
  </div>

  <button
    class="mt-5 rounded-lg bg-teal-800 px-4 py-2 text-sm text-white"
    on:click={save}>Save All Settings</button
  >

  <div class="mt-6 rounded-lg border border-zinc-200 bg-zinc-50 p-4">
    <div class="flex items-center justify-between">
      <div>
        <h3 class="font-semibold text-zinc-900">Export Modpack</h3>
        <p class="text-sm text-[var(--color-muted)]">
          Export currently installed mods as a modpack file
        </p>
      </div>
      <button
        class="rounded-lg bg-teal-700 px-3 py-2 text-xs text-white hover:bg-teal-800 transition"
        on:click={exportInstalledMods}
      >
        Export
      </button>
    </div>
  </div>

  <div class="mt-6 rounded-lg border border-zinc-200 bg-zinc-50 p-4">
    <div class="flex items-center justify-between gap-3">
      <div>
        <h3 class="font-semibold text-zinc-900">App Updates</h3>
        <p class="text-sm text-[var(--color-muted)]">
          {#if updateVersion}
            Update ready: {updateVersion}
          {:else}
            Check GitHub Releases for signed updates
          {/if}
        </p>
      </div>
      <div class="flex gap-2">
        <button
          class="rounded-lg bg-zinc-700 px-3 py-2 text-xs text-white disabled:opacity-50"
          disabled={updateCheckInProgress}
          on:click={checkUpdates}
        >
          {updateCheckInProgress ? "Checking..." : "Check"}
        </button>
        <button
          class="rounded-lg bg-teal-700 px-3 py-2 text-xs text-white disabled:opacity-50"
          disabled={updateInstallInProgress || !updateVersion}
          on:click={installAvailableUpdate}
        >
          {updateInstallInProgress ? "Installing..." : "Install"}
        </button>
      </div>
    </div>
  </div>
</section>
