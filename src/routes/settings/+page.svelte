<script lang="ts">
  import { onMount } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
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
    saveToken,
    setGamePath,
    setModpackUrl,
    setTheme,
    undoIntroSkip,
    validateToken,
    updateConfig,
    verifyNexusApiKey,
  } from "$lib/api/commands";
  import { applyThemeClass } from "$lib/theme";
  import { toastStore } from "$lib/stores/toast";
  import { tokenStore } from "$lib/stores/token";
  import { updateCheckStore } from "$lib/stores/updateCheck";

  const VALIDATION_TTL_MS = 6 * 60 * 60 * 1000;
  const UPDATE_CHECK_COOLDOWN_MS = 15 * 1000;
  const MODIO_VALIDATION_CACHE_KEY = "ronmodmanager.modioValidationCache";
  const NEXUS_VALIDATION_CACHE_KEY = "ronmodmanager.nexusValidationCache";

  type ValidationCache = {
    checkedAt: number;
    valid: boolean;
  };

  let gamePath = "";
  let modpackUrl = "";
  let authConnected = false;
  let hasSavedToken = false;
  let modioTokenValid: boolean | null = null;
  let showTokenModal = false;
  let tokenInput = "";
  let tokenModalError = "";
  let validatingToken = false;
  let nexusApiKey = "";
  let hasNexusKey = false;
  let nexusKeyValid: boolean | null = null;
  let showNexusKeyModal = false;
  let nexusKeyInput = "";
  let validatingNexusKey = false;
  let showNexusKeyText = false;
  let nexusKeyModalError = "";
  let theme: "light" | "dark" | "system" = "system";
  let introSkipApplied = false;
  let applyingIntroSkip = false;
  let undoingIntroSkip = false;
  let updateCheckInProgress = false;
  let updateInstallInProgress = false;
  let updateVersion: string | null = null;
  let updateLastChecked: Date | null = null;

  $: updateLastChecked = $updateCheckStore ? new Date($updateCheckStore) : null;

  function readValidationCache(key: string): ValidationCache | null {
    if (typeof window === "undefined") {
      return null;
    }
    try {
      const raw = window.localStorage.getItem(key);
      if (!raw) {
        return null;
      }
      const parsed = JSON.parse(raw) as Partial<ValidationCache>;
      if (
        typeof parsed.checkedAt !== "number" ||
        typeof parsed.valid !== "boolean"
      ) {
        return null;
      }
      return { checkedAt: parsed.checkedAt, valid: parsed.valid };
    } catch {
      return null;
    }
  }

  function writeValidationCache(key: string, valid: boolean): void {
    if (typeof window === "undefined") {
      return;
    }
    const payload: ValidationCache = { checkedAt: Date.now(), valid };
    window.localStorage.setItem(key, JSON.stringify(payload));
  }

  function clearValidationCache(key: string): void {
    if (typeof window === "undefined") {
      return;
    }
    window.localStorage.removeItem(key);
  }

  function isCacheFresh(cache: ValidationCache | null): boolean {
    if (!cache) {
      return false;
    }
    return Date.now() - cache.checkedAt < VALIDATION_TTL_MS;
  }

  async function refresh() {
    const config = await getConfig();
    nexusApiKey = config.nexus_api_key ?? "";
    hasNexusKey = Boolean(config.nexus_api_key?.trim());
    hasSavedToken = Boolean(config.oauth_token?.trim());
    authConnected = await getAuthStatus().catch(() => false);

    const cachedModio = readValidationCache(MODIO_VALIDATION_CACHE_KEY);
    const cachedNexus = readValidationCache(NEXUS_VALIDATION_CACHE_KEY);

    modioTokenValid = hasSavedToken
      ? isCacheFresh(cachedModio)
        ? (cachedModio?.valid ?? null)
        : null
      : null;
    nexusKeyValid = hasNexusKey
      ? isCacheFresh(cachedNexus)
        ? (cachedNexus?.valid ?? null)
        : null
      : null;

    if (hasSavedToken && !isCacheFresh(cachedModio)) {
      try {
        const valid = await validateToken();
        modioTokenValid = valid;
        authConnected = valid;
        writeValidationCache(MODIO_VALIDATION_CACHE_KEY, valid);
      } catch (error) {
        console.warn("mod.io token validation failed:", error);
      }
    }

    if (hasNexusKey && !isCacheFresh(cachedNexus) && nexusApiKey.trim()) {
      try {
        const valid = await verifyNexusApiKey(nexusApiKey.trim());
        nexusKeyValid = valid;
        writeValidationCache(NEXUS_VALIDATION_CACHE_KEY, valid);
      } catch (error) {
        console.warn("Nexus key validation failed:", error);
      }
    }

    introSkipApplied = await isIntroSkipApplied().catch(() => false);
    gamePath = config.game_path ?? "";
    modpackUrl = config.modpack_url ?? "";
    theme = config.theme;
    applyThemeClass(theme);
  }

  async function autodetect() {
    const detected = await detectGamePath();
    if (!detected) {
      toastStore.error("Could not auto-detect game path.");
      return;
    }
    gamePath = detected;
    await setGamePath(detected);
    toastStore.success("Auto-detected game path.");
  }

  async function persistThemeChoice() {
    try {
      await setTheme(theme);
      applyThemeClass(theme);
    } catch (error) {
      toastStore.error(`Failed to save theme: ${String(error)}`);
    }
  }

  async function save() {
    const errors: string[] = [];

    try {
      await setTheme(theme);
      applyThemeClass(theme);
    } catch (error) {
      errors.push(`Theme: ${String(error)}`);
    }

    try {
      await setGamePath(gamePath.trim());
    } catch (error) {
      errors.push(`Game path: ${String(error)}`);
    }

    try {
      await setModpackUrl(modpackUrl.trim());
    } catch (error) {
      errors.push(`Modpack URL: ${String(error)}`);
    }

    if (errors.length === 0) {
      toastStore.success("Settings saved.");
    } else {
      toastStore.error(`Some settings failed: ${errors.join(" | ")}`);
    }

    await refresh();
  }

  function openTokenSetupModal() {
    tokenInput = "";
    tokenModalError = "";
    showTokenModal = true;
  }

  function closeTokenSetupModal() {
    tokenInput = "";
    tokenModalError = "";
    showTokenModal = false;
  }

  async function openModioTokenPage() {
    try {
      await openUrl("https://mod.io/me/access");
    } catch (error) {
      toastStore.error(`Failed to open mod.io token page: ${String(error)}`);
    }
  }

  async function validateAndSaveToken() {
    const trimmed = tokenInput.trim();
    if (!trimmed) {
      tokenModalError = "Please paste a token before validating.";
      return;
    }

    validatingToken = true;
    tokenModalError = "";

    try {
      await saveToken(trimmed);
      const valid = await validateToken();
      if (!valid) {
        writeValidationCache(MODIO_VALIDATION_CACHE_KEY, false);
        await logout();
        tokenStore.set(false);
        tokenModalError =
          "Token is invalid or expired. Please generate a new token and try again.";
        return;
      }

      writeValidationCache(MODIO_VALIDATION_CACHE_KEY, true);
      tokenStore.set(true);
      await refresh();
      closeTokenSetupModal();
      toastStore.success("Token validated and saved.");
    } catch (error) {
      tokenModalError = `Validation failed: ${String(error)}`;
    } finally {
      validatingToken = false;
    }
  }

  async function disconnect() {
    await logout();
    clearValidationCache(MODIO_VALIDATION_CACHE_KEY);
    tokenStore.set(false);
    toastStore.success("Logged out.");
    await refresh();
  }

  function openNexusKeyModal() {
    nexusKeyInput = nexusApiKey;
    showNexusKeyModal = true;
    showNexusKeyText = false;
    nexusKeyModalError = "";
  }

  function closeNexusKeyModal() {
    nexusKeyInput = "";
    showNexusKeyModal = false;
    showNexusKeyText = false;
    nexusKeyModalError = "";
  }

  async function openNexusApiKeysPage() {
    try {
      await openUrl("https://www.nexusmods.com/settings/api-keys");
    } catch (error) {
      toastStore.error(`Failed to open Nexus API keys page: ${String(error)}`);
    }
  }

  async function saveNexusKey() {
    const trimmed = nexusKeyInput.trim();
    nexusKeyModalError = "";

    // If removing the key, just save and return
    if (!trimmed) {
      try {
        await updateConfig({ nexus_api_key: null });
        clearValidationCache(NEXUS_VALIDATION_CACHE_KEY);
        nexusKeyValid = null;
        await refresh();
        closeNexusKeyModal();
        toastStore.success("Nexus API key removed.");
      } catch (error) {
        nexusKeyModalError = `Failed to remove key: ${String(error)}`;
        toastStore.error(`Failed to remove Nexus API key: ${String(error)}`);
      }
      return;
    }

    // Validate the key before saving
    validatingNexusKey = true;
    try {
      const isValid = await verifyNexusApiKey(trimmed);

      if (!isValid) {
        writeValidationCache(NEXUS_VALIDATION_CACHE_KEY, false);
        nexusKeyValid = false;
        nexusKeyModalError =
          "Invalid Nexus API key. Please check and try again.";
        toastStore.error("Invalid Nexus API key. Please check and try again.");
        validatingNexusKey = false;
        return;
      }

      // Key is valid, save it
      await updateConfig({ nexus_api_key: trimmed });
      writeValidationCache(NEXUS_VALIDATION_CACHE_KEY, true);
      nexusKeyValid = true;
      await refresh();
      closeNexusKeyModal();
      toastStore.success("Nexus API key validated and saved successfully!");
    } catch (error) {
      nexusKeyModalError = `Validation failed: ${String(error)}`;
      toastStore.error(`Failed to validate Nexus API key: ${String(error)}`);
    } finally {
      validatingNexusKey = false;
    }
  }

  async function applyIntroSkipConfig() {
    applyingIntroSkip = true;
    try {
      await applyIntroSkip();
      introSkipApplied = true;
      toastStore.success("Intro skip applied successfully!");
    } catch (error) {
      toastStore.error(`Failed to apply intro skip: ${error}`);
    } finally {
      applyingIntroSkip = false;
      await refresh();
    }
  }

  async function undoIntroSkipConfig() {
    undoingIntroSkip = true;
    try {
      await undoIntroSkip();
      introSkipApplied = false;
      toastStore.success("Intro skip reverted.");
    } catch (error) {
      toastStore.error(`Failed to undo intro skip: ${error}`);
    } finally {
      undoingIntroSkip = false;
      await refresh();
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
      toastStore.success(
        `Modpack exported successfully! Filename: ${filename}`,
      );
    } catch (error) {
      toastStore.error(`Failed to export modpack: ${error}`);
    }
  }

  async function checkUpdates() {
    const lastCheckedAt = $updateCheckStore;
    if (lastCheckedAt) {
      const cooldownRemaining =
        UPDATE_CHECK_COOLDOWN_MS - (Date.now() - lastCheckedAt);
      if (cooldownRemaining > 0) {
        const secondsRemaining = Math.ceil(cooldownRemaining / 1000);
        import("$lib/stores/operationStatus").then(
          ({ operationStatusStore }) => {
            operationStatusStore.setTemporaryMessage(
              `Please wait ${secondsRemaining}s before checking again.`,
              2500,
            );
          },
        );
        return;
      }
    }

    updateCheckInProgress = true;
    try {
      const info = await checkForUpdate();
      if (info.available) {
        updateVersion = info.version;
        toastStore.success(`Update available: ${info.version}`);
      } else {
        updateVersion = null;
        toastStore.info("No updates available.");
      }
    } catch (error) {
      toastStore.error(`Failed to check updates: ${error}`);
    } finally {
      updateCheckInProgress = false;
      // Store timestamp of this check to show in UI and throttle rapid re-checks.
      updateCheckStore.markChecked();
    }
  }

  async function installAvailableUpdate() {
    updateInstallInProgress = true;
    try {
      const info = await installUpdate();
      if (info.available) {
        toastStore.success(
          "Update installed. The app may close automatically on Windows; otherwise restart to use the new version.",
        );
      } else {
        toastStore.info("No update available to install.");
      }
    } catch (error) {
      toastStore.error(`Failed to install update: ${error}`);
    } finally {
      updateInstallInProgress = false;
    }
  }

  onMount(() => {
    void refresh();
  });
</script>

<section class="card">
  <h1 style="color: var(--clr-text);" class="mb-4 text-2xl font-semibold">
    Settings
  </h1>

  <div class="mt-4 grid gap-4 md:grid-cols-2">
    <label class="block text-sm">
      <span style="color: var(--clr-text-secondary);" class="mb-1 block"
        >Game Path</span
      >
      <input class="input w-full" bind:value={gamePath} />
      <button class="btn btn-sm mt-2" on:click={autodetect}>Auto Detect</button>
    </label>

    <label class="block text-sm">
      <span style="color: var(--clr-text-secondary);" class="mb-1 block"
        >Modpack URL</span
      >
      <input class="input w-full" bind:value={modpackUrl} />
    </label>
  </div>

  <div class="card mt-4">
    <div class="flex items-center justify-between gap-3">
      <div>
        <h3 style="color: var(--clr-text);" class="font-semibold">
          Nexus Mods API Key
        </h3>
        <p style="color: var(--clr-text-secondary);" class="text-sm mb-2">
          <strong>Optional:</strong> Configure your Nexus Mods API key to fetch mod
          metadata (name, description) when adding Nexus mods. Without a key, you
          can still add mods manually.
        </p>
        <p style="color: var(--clr-text-secondary);" class="text-sm">
          Status:
          <span
            style="color: {hasNexusKey
              ? nexusKeyValid === false
                ? 'var(--clr-danger-300)'
                : nexusKeyValid === true
                  ? 'var(--clr-success-300)'
                  : 'var(--clr-primary-300)'
              : 'var(--clr-danger-300)'};"
            class="font-medium"
          >
            {hasNexusKey
              ? nexusKeyValid === true
                ? "✓ Configured"
                : nexusKeyValid === false
                  ? "⚠ Configured, validation failed"
                  : "⚠ Configured, pending check"
              : "✗ Not configured"}
          </span>
        </p>
      </div>
      <div class="flex gap-2">
        <button class="btn btn-sm primary" on:click={openNexusKeyModal}>
          {hasNexusKey ? "Update Key" : "Set Key"}
        </button>
        {#if hasNexusKey}
          <button
            class="btn btn-sm danger"
            on:click={async () => {
              nexusKeyInput = "";
              await saveNexusKey();
            }}
          >
            Remove Key
          </button>
        {/if}
      </div>
    </div>
  </div>

  <div class="card mt-4">
    <label class="block text-sm">
      <span style="color: var(--clr-text-secondary);" class="mb-1 block"
        >Theme</span
      >
      <select
        class="select w-full"
        bind:value={theme}
        on:change={() => {
          applyThemeClass(theme);
          void persistThemeChoice();
        }}
      >
        <option value="system">System</option>
        <option value="light">Light</option>
        <option value="dark">Dark</option>
      </select>
    </label>
  </div>

  <div class="card mt-4">
    <div class="flex items-center justify-between gap-3">
      <div>
        <h3 style="color: var(--clr-text);" class="font-semibold">
          mod.io API Token
        </h3>
        <p style="color: var(--clr-text-secondary);" class="text-sm">
          Status:
          <span
            style="color: {hasSavedToken
              ? modioTokenValid === false
                ? 'var(--clr-danger-300)'
                : modioTokenValid === true
                  ? 'var(--clr-success-300)'
                  : 'var(--clr-primary-300)'
              : 'var(--clr-danger-300)'};"
            class="font-medium"
          >
            {hasSavedToken
              ? modioTokenValid === true
                ? "✓ Configured"
                : modioTokenValid === false
                  ? "⚠ Configured, validation failed"
                  : "⚠ Configured, pending check"
              : "✗ Not configured"}
          </span>
        </p>
      </div>
      <div class="flex gap-2">
        <button class="btn btn-sm primary" on:click={openTokenSetupModal}
          >Set Token</button
        >
        <button
          class="btn btn-sm danger"
          disabled={!hasSavedToken}
          on:click={disconnect}>Remove Token</button
        >
      </div>
    </div>
  </div>

  <div class="card mt-4">
    <div class="flex items-center justify-between">
      <div>
        <h3 style="color: var(--clr-text);" class="font-semibold">
          Intro Skip
        </h3>
        <p style="color: var(--clr-text-secondary);" class="text-sm">
          {introSkipApplied
            ? "✓ Applied"
            : "Configure Game.ini to skip intro video"}
        </p>
      </div>
      <div class="flex gap-2">
        <button
          class="btn btn-sm primary"
          class:disabled={applyingIntroSkip || introSkipApplied}
          disabled={applyingIntroSkip || introSkipApplied}
          on:click={applyIntroSkipConfig}
        >
          {applyingIntroSkip
            ? "Applying..."
            : introSkipApplied
              ? "Applied"
              : "Apply"}
        </button>
        <button
          class="btn btn-sm"
          class:disabled={undoingIntroSkip || !introSkipApplied}
          disabled={undoingIntroSkip || !introSkipApplied}
          on:click={undoIntroSkipConfig}
        >
          {undoingIntroSkip ? "Undoing..." : "Undo"}
        </button>
      </div>
    </div>
  </div>

  <button class="btn primary mt-5" on:click={save}>Save All Settings</button>

  <div class="card mt-6">
    <div class="flex items-center justify-between">
      <div>
        <h3 style="color: var(--clr-text);" class="font-semibold">
          Export Modpack
        </h3>
        <p style="color: var(--clr-text-secondary);" class="text-sm">
          Export currently installed mods as a modpack file
        </p>
      </div>
      <button class="btn btn-sm primary" on:click={exportInstalledMods}>
        Export
      </button>
    </div>
  </div>

  <div class="card mt-6">
    <div class="flex items-center justify-between gap-3">
      <div>
        <h3 style="color: var(--clr-text);" class="font-semibold">
          App Updates
        </h3>
        <p style="color: var(--clr-text-secondary);" class="text-sm">
          {#if updateVersion}
            Update ready: {updateVersion}
          {:else}
            Check for updates
          {/if}
        </p>
        <!-- Muted last checked text -->
        {#if updateLastChecked}
          <p style="color: var(--clr-text-secondary);" class="text-xs">
            Last checked: {updateLastChecked.toLocaleString()}
          </p>
        {/if}
      </div>
      <div class="flex gap-2">
        <button
          class="btn btn-sm primary"
          class:disabled={updateCheckInProgress || updateInstallInProgress}
          disabled={updateCheckInProgress || updateInstallInProgress}
          on:click={updateVersion ? installAvailableUpdate : checkUpdates}
        >
          {#if updateCheckInProgress}
            Checking...
          {:else if updateInstallInProgress}
            Installing...
          {:else if updateVersion}
            Install Update
          {:else}
            Check for Updates
          {/if}
        </button>
      </div>
    </div>
  </div>
</section>

{#if showTokenModal}
  <div
    class="fixed inset-0 z-[1200] flex items-center justify-center p-4"
    style="background: rgba(0, 0, 0, 0.65);"
  >
    <div class="card w-full max-w-xl">
      <h2 style="color: var(--clr-text);" class="text-lg font-semibold">
        Set mod.io API Token
      </h2>
      <p style="color: var(--clr-text-secondary);" class="text-sm mt-2">
        Create a token on mod.io with any name and enable both <strong
          >Read</strong
        >
        and <strong>Write</strong> permissions.
      </p>

      <div class="mt-4 flex flex-wrap gap-2">
        <button class="btn btn-sm" on:click={openModioTokenPage}
          >Open API Token Page</button
        >
      </div>

      <label class="mt-4 block text-sm">
        <span style="color: var(--clr-text-secondary);" class="mb-1 block"
          >Paste token</span
        >
        <input
          class="input w-full"
          bind:value={tokenInput}
          placeholder="Paste your mod.io API token"
        />
      </label>

      {#if tokenModalError}
        <p class="mt-3 text-sm" style="color: var(--clr-danger-300);">
          {tokenModalError}
        </p>
      {/if}

      <div class="mt-5 flex justify-end gap-2">
        <button
          class="btn btn-sm"
          on:click={closeTokenSetupModal}
          disabled={validatingToken}>Cancel</button
        >
        <button
          class="btn btn-sm primary"
          on:click={validateAndSaveToken}
          disabled={validatingToken}
        >
          {validatingToken ? "Validating..." : "Validate and Save"}
        </button>
      </div>
    </div>
  </div>
{/if}

{#if showNexusKeyModal}
  <div
    class="fixed inset-0 z-[1200] flex items-center justify-center p-4"
    style="background: rgba(0, 0, 0, 0.65);"
  >
    <div class="card w-full max-w-xl">
      <h2 style="color: var(--clr-text);" class="text-lg font-semibold">
        Set Nexus Mods API Key
      </h2>
      <p style="color: var(--clr-text-secondary);" class="text-sm mt-2">
        Get your <strong>Personal API Key</strong> from Nexus Mods to fetch mod information.
      </p>

      <div
        style="background: color-mix(in srgb, var(--clr-primary-300) 15%, transparent); border-left: 3px solid var(--clr-primary-300);"
        class="mt-3 p-3 rounded"
      >
        <p style="color: var(--clr-text);" class="text-sm font-medium">
          Important:
        </p>
        <p style="color: var(--clr-text-secondary);" class="text-xs mt-1">
          On the Nexus API keys page, <strong
            >scroll all the way to the bottom</strong
          > to find your "Personal API Key" section. Copy the key from there.
        </p>
      </div>

      <div class="mt-4 flex flex-wrap gap-2">
        <button class="btn btn-sm" on:click={openNexusApiKeysPage}
          >Open Nexus API Keys Page</button
        >
      </div>

      <label class="mt-4 block text-sm">
        <span style="color: var(--clr-text-secondary);" class="mb-1 block"
          >Paste API key</span
        >
        <div class="flex gap-2">
          <input
            class="input w-full"
            bind:value={nexusKeyInput}
            on:input={() => (nexusKeyModalError = "")}
            placeholder="Paste your Nexus Mods Personal API Key"
            type={showNexusKeyText ? "text" : "password"}
            aria-invalid={Boolean(nexusKeyModalError)}
          />
          <button
            type="button"
            class="btn btn-sm"
            on:click={() => (showNexusKeyText = !showNexusKeyText)}
            title={showNexusKeyText ? "Hide key" : "Show key"}
          >
            {showNexusKeyText ? "👁️" : "👁️‍🗨️"}
          </button>
        </div>
      </label>

      {#if nexusKeyModalError}
        <p class="mt-3 text-sm" style="color: var(--clr-danger-300);">
          {nexusKeyModalError}
        </p>
      {/if}

      <div class="mt-5 flex justify-end gap-2">
        <button
          class="btn btn-sm"
          on:click={closeNexusKeyModal}
          disabled={validatingNexusKey}>Cancel</button
        >
        <button
          class="btn btn-sm primary"
          on:click={saveNexusKey}
          disabled={validatingNexusKey}
        >
          {validatingNexusKey ? "Validating..." : "Verify & Save"}
        </button>
      </div>
    </div>
  </div>
{/if}
