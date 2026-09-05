<script lang="ts">
  import {
    applyIntroSkip,
    applyOptimization,
    detectGpuProfile,
    disableOptimization,
    getGpuProfiles,
    buildModpackFromInstalled,
    checkForUpdate,
    detectGamePath,
    exportModpackToFile,
    getAuthStatus,
    getConfig,
    getModpackMeta,
    getSyncDetails,
    installUpdate,
    isRunningInFlatpak,
    isIntroSkipApplied,
    logout,
    revealInFileManager,
    setGamePath,
    setModpackMeta,
    setSyncDetails,
    setTheme,
    syncModpackToRemote,
    undoIntroSkip,
    updateConfig,
    validateToken,
    verifyNexusApiKey,
  } from "$lib/api/commands";
  import {
    validateAndSaveModioApiKey,
    validateAndSaveModioToken,
    validateAndSaveNexusApiKey,
  } from "$lib/api/apiKeyValidation";
  import ExportModpackModal from "$lib/components/ExportModpackModal.svelte";
  import SyncAuthModal from "$lib/components/SyncAuthModal.svelte";
  import { syncLogStore } from "$lib/stores/syncLogStore";
  import type {
    CloseAction,
    MinimizeTarget,
    OnGameLaunchAction,
    SyncAuth,
  } from "$lib/types";
  import { listen } from "@tauri-apps/api/event";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import type { ModProgressEvent } from "$lib/types";
  import { operationStatusStore } from "$lib/stores/operationStatus";
  import { toastStore } from "$lib/stores/toast";
  import { tokenStore } from "$lib/stores/token";
  import { updateCheckStore } from "$lib/stores/updateCheck";
  import { screenshotMode } from "$lib/stores/incognitoMode";
  import { applyThemeClass } from "$lib/theme";
  import { getVersion } from "@tauri-apps/api/app";
  import { downloadDir } from "@tauri-apps/api/path";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { onDestroy, onMount } from "svelte";
  // Persist modpack export metadata per-profile (backend), not localStorage
  let showExportModal = false;
  let exportMeta = {
    name: "",
    version: "",
    description: "",
    author: "",
  };

  const VALIDATION_TTL_MS = 6 * 60 * 60 * 1000;
  const UPDATE_CHECK_COOLDOWN_MS = 15 * 1000;
  const MODIO_VALIDATION_CACHE_KEY = "ronmodmanager.modioValidationCache";
  const NEXUS_VALIDATION_CACHE_KEY = "ronmodmanager.nexusValidationCache";

  type ValidationCache = {
    checkedAt: number;
    valid: boolean;
  };

  let gamePath = "";
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
  let modioApiKey = "";
  let hasModioApiKey = false;
  let showModioApiKeyModal = false;
  let modioApiKeyInput = "";
  let validatingModioApiKey = false;
  let showModioApiKeyText = false;
  let modioApiKeyModalError = "";
  let theme: "light" | "dark" | "system" = "system";
  let introSkipApplied = false;
  let applyingIntroSkip = false;
  let undoingIntroSkip = false;
  let gpuProfiles: string[] = [];
  let selectedGpu = "";
  let detectedGpu: string | null = null;
  let optimizationProfile: string | null = null;
  let optimizationEnabled = false;
  let applyingOpt = false;
  let runningInFlatpak = false;
  let currentVersion = "";
  let updateCheckInProgress = false;
  let updateInstallInProgress = false;
  let updateVersion: string | null = null;
  let updateLastChecked: Date | null = null;
  let syncRemoteHost = "";
  let syncRemotePath = "";
  let showSyncAuthModal = false;
  let syncVerbose = false;
  let syncAuthPurpose: "manual" | "fallback" = "manual";
  let onGameLaunch: OnGameLaunchAction = "nothing";
  let closeAction: CloseAction = "quit";
  let minimizeTarget: MinimizeTarget = "taskbar";

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
    modioApiKey =
      typeof config.modio_api_key === "string"
        ? config.modio_api_key.trim()
        : "";
    hasModioApiKey = !!modioApiKey;
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
    optimizationEnabled = config.optimization_enabled ?? false;
    optimizationProfile = config.optimization_profile ?? null;
    selectedGpu = optimizationProfile ?? "";
    gpuProfiles = await getGpuProfiles().catch(() => []);
    detectedGpu = await detectGpuProfile().catch(() => null);
    if (!selectedGpu && detectedGpu) selectedGpu = detectedGpu;
    gamePath = config.game_path ?? "";
    theme = config.theme;
    if (!$screenshotMode) applyThemeClass(theme);
    await loadSyncDetails();
    onGameLaunch = config.on_game_launch ?? "nothing";
    closeAction = config.close_action ?? "quit";
    minimizeTarget = config.minimize_target ?? "taskbar";
  }
  function openModioApiKeyModal() {
    modioApiKeyInput = modioApiKey;
    showModioApiKeyModal = true;
    showModioApiKeyText = false;
    modioApiKeyModalError = "";
  }

  function closeModioApiKeyModal() {
    modioApiKeyInput = "";
    showModioApiKeyModal = false;
    showModioApiKeyText = false;
    modioApiKeyModalError = "";
  }

  async function openModioApiKeyPage() {
    try {
      await openUrl("https://mod.io/me/access");
    } catch (error) {
      toastStore.error(`Failed to open mod.io API key page: ${String(error)}`);
    }
  }

  async function saveModioApiKey() {
    const trimmed = modioApiKeyInput.trim();
    modioApiKeyModalError = "";

    if (!trimmed) {
      try {
        await updateConfig({ modio_api_key: "" });
        await refresh();
        closeModioApiKeyModal();
        toastStore.success("mod.io API Access key removed.");
      } catch (error) {
        modioApiKeyModalError = `Failed to remove key: ${String(error)}`;
        toastStore.error(
          `Failed to remove mod.io API Access key: ${String(error)}`,
        );
      }
      return;
    }

    validatingModioApiKey = true;
    try {
      const ok = await validateAndSaveModioApiKey(trimmed);
      if (!ok) {
        modioApiKeyModalError =
          "API key validation failed. Please check the key and try again.";
        toastStore.error("mod.io API key validation failed.");
        return;
      }
      await refresh();
      closeModioApiKeyModal();
      toastStore.success(
        "mod.io API Access key validated and saved successfully!",
      );
    } catch (error) {
      modioApiKeyModalError = `Failed to validate/save key: ${String(error)}`;
      toastStore.error(
        `Failed to validate/save mod.io API Access key: ${String(error)}`,
      );
    } finally {
      validatingModioApiKey = false;
    }
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
      const valid = await validateAndSaveModioToken(trimmed);
      if (!valid) {
        writeValidationCache(MODIO_VALIDATION_CACHE_KEY, false);
        await logout();
        tokenStore.set(false);
        tokenModalError =
          "Token is invalid or expired. Please generate a new token and try again.";
        return;
      }

      writeValidationCache(MODIO_VALIDATION_CACHE_KEY, true);
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

    if (!trimmed) {
      try {
        await updateConfig({ nexus_api_key: "" });
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

    validatingNexusKey = true;
    try {
      const ok = await validateAndSaveNexusApiKey(trimmed);

      if (!ok) {
        writeValidationCache(NEXUS_VALIDATION_CACHE_KEY, false);
        nexusKeyValid = false;
        nexusKeyModalError =
          "Invalid Nexus API key. Please check and try again.";
        toastStore.error("Invalid Nexus API key. Please check and try again.");
        return;
      }

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

  async function applyOpt() {
    if (!selectedGpu) return;
    applyingOpt = true;
    try {
      await applyOptimization(selectedGpu);
      toastStore.success(`Optimization applied: ${selectedGpu}`);
      await refresh();
    } catch (e) {
      toastStore.error(String(e));
    } finally {
      applyingOpt = false;
    }
  }
  async function removeOpt() {
    applyingOpt = true;
    try {
      await disableOptimization();
      toastStore.success("Optimization removed");
      await refresh();
    } catch (e) {
      toastStore.error(String(e));
    } finally {
      applyingOpt = false;
    }
  }

  async function exportInstalledMods() {
    try {
      const meta = await getModpackMeta().catch(() => null);
      exportMeta = {
        name: meta?.name ?? "",
        version: meta?.version ?? "",
        description: meta?.description ?? "",
        author: meta?.author ?? "",
      };
    } catch (error) {
      console.warn("Failed to load modpack metadata for profile:", error);
    }
    showExportModal = true;
  }

  async function runSync(auth?: SyncAuth) {
    await setSyncDetails(syncRemoteHost.trim(), syncRemotePath.trim());
    syncLogStore.start();

    let unlisten: UnlistenFn | null = null;
    try {
      unlisten = await listen<ModProgressEvent>("sync_progress", (event) => {
        if (event.payload.operation !== "sync_uploading") {
          syncLogStore.addLine(event.payload.message);
        }
      });

      await syncModpackToRemote(auth, syncVerbose);
    } catch (error) {
      const msg = String(error);
      if (msg.includes("AUTH_REQUIRED")) {
        syncLogStore.close();
        syncAuthPurpose = "fallback";
        showSyncAuthModal = true;
      } else {
        syncLogStore.addLine(`Error: ${msg}`);
      }
    } finally {
      syncLogStore.finish();
      unlisten?.();
    }
  }

  function handleSync() {
    void runSync();
  }

  function openSyncAuthManual() {
    syncAuthPurpose = "manual";
    showSyncAuthModal = true;
  }

  function handleSyncAuthSubmit(_auth: SyncAuth) {
    showSyncAuthModal = false;
    toastStore.success("Credentials saved (tested). Use Sync Now to sync.");
  }

  async function loadSyncDetails() {
    const syncDetails = await getSyncDetails().catch(() => null);
    syncRemoteHost = syncDetails?.host ?? "";
    syncRemotePath = syncDetails?.path ?? "";
  }

  function handleProfileChanged() {
    void loadSyncDetails();
  }

  async function handleExportModpack({
    name,
    version,
    description,
    author,
  }: {
    name: string;
    version: string;
    description: string;
    author: string;
  }) {
    showExportModal = false;
    exportMeta = { name, version, description, author };
    setModpackMeta(name, version, description, author || null).catch(
      (error) => {
        console.warn("Failed to save modpack metadata to profile:", error);
      },
    );
    let exportDir = "";
    try {
      let modpack = await buildModpackFromInstalled();
      // Overwrite metadata
      modpack = {
        ...modpack,
        name,
        version,
        description,
        author,
      };
      const folderName = `${name.replace(/[^a-zA-Z0-9_-]+/g, "_")}-${version.replace(/[^a-zA-Z0-9._-]+/g, "_")}`;
      let downloadsPath = "~/Downloads";
      try {
        downloadsPath = await downloadDir();
      } catch (e) {
        // fallback to ~/Downloads
      }
      exportDir = `${downloadsPath.replace(/\/$/, "")}/${folderName}`;
      console.log("Exporting modpack to:", exportDir);
      operationStatusStore.setTemporaryMessage("Exporting modpack...", 10000);
      try {
        await exportModpackToFile(modpack, exportDir);
        console.log("Export successful!");
        const toastId = toastStore.add(
          `Modpack exported to: ${exportDir}<br><span class='text-xs opacity-80'>Contains modpack.json and mods/</span> <button class='btn btn-xs ml-2' onclick='window.__OPEN_EXPORT_DIR && window.__OPEN_EXPORT_DIR()'>Open Folder</button>`,
          "success",
          0,
        );
        // @ts-ignore
        window.__OPEN_EXPORT_DIR = () => {
          revealInFileManager(exportDir);
          toastStore.remove(toastId);
        };
      } catch (error) {
        console.error("Export failed:", error);
        operationStatusStore.clear();
        toastStore.error(`Failed to export modpack: ${error}`);
      }
    } catch (error) {
      console.error("Modpack build failed:", error);
      operationStatusStore.clear();
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

  async function migrateLegacyModpackMeta() {
    if (typeof window === "undefined") return;
    const LEGACY_KEY = "ronmodmanager.modpackMeta";
    const raw = window.localStorage.getItem(LEGACY_KEY);
    if (!raw) return;
    window.localStorage.removeItem(LEGACY_KEY);
    try {
      const parsed = JSON.parse(raw);
      if (parsed?.name) {
        await setModpackMeta(
          parsed.name,
          parsed.version ?? "1.0.0",
          parsed.description ?? null,
          parsed.author || null,
        );
      }
    } catch (error) {
      console.warn("Failed to migrate legacy modpack metadata:", error);
    }
  }

  onMount(async () => {
    window.addEventListener("ron:profile-changed", handleProfileChanged);
    runningInFlatpak = await isRunningInFlatpak();
    currentVersion = await getVersion();
    void migrateLegacyModpackMeta();
    void refresh();
  });

  onDestroy(() => {
    window.removeEventListener("ron:profile-changed", handleProfileChanged);
  });
</script>

<section class="prefs-page">
  <h1 style="color: var(--clr-text);" class="mb-2 text-2xl font-semibold">
    Settings
  </h1>

  <!-- General -->
  <div class="prefs-group">
    <div class="prefs-group-title">General</div>
    <div class="prefs-boxed-list">
      <div class="prefs-row">
        <div class="prefs-row-text">
          <div class="prefs-row-title">Game Path</div>
          <div class="prefs-row-subtitle">ReadyOrNot installation folder</div>
        </div>
        <div class="prefs-row-suffix flex gap-2 flex-1 max-w-[420px]">
          <input
            class="input flex-1"
            bind:value={gamePath}
            on:blur={async () => {
              try {
                await setGamePath(gamePath.trim());
              } catch (e) {
                toastStore.error(`Game path: ${String(e)}`);
              }
            }}
          /><button class="btn btn-sm" on:click={autodetect}>Auto Detect</button
          >
        </div>
      </div>
      <div class="prefs-row">
        <div class="prefs-row-text">
          <div class="prefs-row-title">Theme</div>
        </div>
        <div class="prefs-row-suffix">
          <select
            class="select w-40"
            bind:value={theme}
            on:change={() => {
              applyThemeClass(theme);
              void persistThemeChoice();
            }}
            ><option value="system">System</option><option value="light"
              >Light</option
            ><option value="dark">Dark</option></select
          >
        </div>
      </div>
      <div class="prefs-row">
        <div class="prefs-row-text">
          <div class="prefs-row-title">When launching game</div>
        </div>
        <div class="prefs-row-suffix">
          <select
            class="select w-40"
            bind:value={onGameLaunch}
            on:change={() =>
              void updateConfig({ on_game_launch: onGameLaunch })}
            ><option value="nothing">Do nothing</option><option value="minimize"
              >Minimise</option
            ><option value="close">Quit</option></select
          >
        </div>
      </div>
      <div class="prefs-row">
        <div class="prefs-row-text">
          <div class="prefs-row-title">When closing window</div>
        </div>
        <div class="prefs-row-suffix">
          <select
            class="select w-40"
            bind:value={closeAction}
            on:change={() =>
              void updateConfig({
                close_action: closeAction,
                asked_close_preference: true,
              })}
            ><option value="quit">Quit</option><option value="minimize"
              >Minimise</option
            ></select
          >
        </div>
      </div>
      <div
        class="prefs-row"
        class:opacity-50={onGameLaunch === "nothing" && closeAction === "quit"}
      >
        <div class="prefs-row-text">
          <div class="prefs-row-title">Minimise to</div>
        </div>
        <div class="prefs-row-suffix">
          <select
            class="select w-40"
            bind:value={minimizeTarget}
            disabled={onGameLaunch === "nothing" && closeAction === "quit"}
            on:change={() =>
              void updateConfig({
                minimize_target: minimizeTarget,
                asked_close_preference: true,
              })}
            ><option value="taskbar">Taskbar</option><option value="tray"
              >System tray</option
            ></select
          >
        </div>
      </div>
    </div>
  </div>

  <!-- Accounts -->
  <div class="prefs-group">
    <div class="prefs-group-title">Accounts & API Keys</div>
    <div class="prefs-group-desc">
      Optional keys for fetching mod metadata. OAuth is required to
      subscribe/download.
    </div>
    <div class="prefs-boxed-list">
      <div class="prefs-row">
        <div class="prefs-row-text">
          <div class="prefs-row-title">Nexus Mods API Key</div>
          <div
            class="prefs-row-subtitle"
            style="color:{hasNexusKey
              ? nexusKeyValid === true
                ? 'var(--clr-success-300)'
                : nexusKeyValid === false
                  ? 'var(--clr-danger-300)'
                  : 'var(--clr-text-secondary)'
              : 'var(--clr-danger-300)'}"
          >
            {hasNexusKey
              ? nexusKeyValid === true
                ? "✓ Configured"
                : nexusKeyValid === false
                  ? "⚠ Validation failed"
                  : "⚠ Pending check"
              : "✗ Not configured"}
          </div>
        </div>
        <div class="prefs-row-suffix">
          <button class="btn btn-sm primary" on:click={openNexusKeyModal}
            >{hasNexusKey ? "Update Key" : "Set Key"}</button
          >{#if hasNexusKey}<button
              class="btn btn-sm danger"
              on:click={async () => {
                nexusKeyInput = "";
                await saveNexusKey();
              }}>Remove</button
            >{/if}
        </div>
      </div>
      <div class="prefs-row">
        <div class="prefs-row-text">
          <div class="prefs-row-title">mod.io OAuth Access</div>
          <div
            class="prefs-row-subtitle"
            style="color:{hasSavedToken
              ? modioTokenValid === true
                ? 'var(--clr-success-300)'
                : modioTokenValid === false
                  ? 'var(--clr-danger-300)'
                  : 'var(--clr-text-secondary)'
              : 'var(--clr-danger-300)'}"
          >
            {hasSavedToken
              ? modioTokenValid === true
                ? "✓ Configured"
                : modioTokenValid === false
                  ? "⚠ Validation failed"
                  : "⚠ Pending check"
              : "✗ Not configured"}
          </div>
        </div>
        <div class="prefs-row-suffix">
          <button class="btn btn-sm primary" on:click={openTokenSetupModal}
            >Set OAuth</button
          ><button
            class="btn btn-sm danger"
            disabled={!hasSavedToken}
            on:click={disconnect}>Remove</button
          >
        </div>
      </div>
      <div class="prefs-row">
        <div class="prefs-row-text">
          <div class="prefs-row-title">mod.io API Access</div>
          <div
            class="prefs-row-subtitle"
            style="color:{hasModioApiKey
              ? 'var(--clr-success-300)'
              : 'var(--clr-danger-300)'}"
          >
            {hasModioApiKey ? "✓ Configured" : "✗ Not configured"}
          </div>
        </div>
        <div class="prefs-row-suffix">
          <button class="btn btn-sm primary" on:click={openModioApiKeyModal}
            >{hasModioApiKey ? "Update" : "Set Key"}</button
          >{#if hasModioApiKey}<button
              class="btn btn-sm danger"
              on:click={async () => {
                modioApiKeyInput = "";
                await saveModioApiKey();
              }}>Remove</button
            >{/if}
        </div>
      </div>
    </div>
  </div>

  <!-- Tweaks -->
  <div class="prefs-group">
    <div class="prefs-group-title">Game Tweaks</div>
    <div class="prefs-boxed-list">
      <div class="prefs-row">
        <div class="prefs-row-text">
          <div class="prefs-row-title">Intro Skip</div>
          <div class="prefs-row-subtitle">Removes startup movies</div>
        </div>
        <div class="prefs-row-suffix">
          <label
            class="gale-switch"
            class:opacity-50={applyingIntroSkip || undoingIntroSkip}
            ><input
              type="checkbox"
              checked={introSkipApplied}
              disabled={applyingIntroSkip || undoingIntroSkip}
              on:change={() => {
                if (introSkipApplied) void undoIntroSkipConfig();
                else void applyIntroSkipConfig();
              }}
            /><span class="gale-switch-track"></span></label
          >
        </div>
      </div>
      <div class="prefs-row">
        <div class="prefs-row-text" style="flex:1">
          <div class="prefs-row-title">Engine.ini Optimization</div>
          <div class="prefs-row-subtitle">
            UE5 Performance Overhaul by <a
              href="https://www.nexusmods.com/readyornot/mods/5845"
              target="_blank"
              style="color:var(--clr-primary-300);text-decoration:underline;"
              >AlexRenderX</a
            >
          </div>
        </div>
        <div class="prefs-row-suffix">
          <select class="select w-48" bind:value={selectedGpu}
            ><option value="">Select GPU…</option
            >{#each gpuProfiles as p}<option value={p}
                >{p.replace(/_/g, " ").replace(/-/g, " / ")}</option
              >{/each}</select
          ><button
            class="btn btn-sm primary"
            disabled={!selectedGpu || applyingOpt}
            on:click={applyOpt}>{applyingOpt ? "…" : "Apply"}</button
          >{#if optimizationEnabled}<button
              class="btn btn-sm"
              disabled={applyingOpt}
              on:click={removeOpt}>Restore</button
            >{/if}
        </div>
      </div>
    </div>
  </div>

  <!-- Sync & Export -->
  <div class="prefs-group">
    <div class="prefs-group-title">Sync & Export</div>
    <div class="prefs-boxed-list">
      <div class="prefs-row">
        <div class="prefs-row-text">
          <div class="prefs-row-title">Export Modpack</div>
          <div class="prefs-row-subtitle">Export installed mods as modpack</div>
        </div>
        <div class="prefs-row-suffix">
          <button class="btn btn-sm primary" on:click={exportInstalledMods}
            >Export</button
          >
        </div>
      </div>
      <div class="prefs-row" style="flex-direction:column;align-items:stretch;">
        <div class="prefs-row-title">
          Remote Sync <span
            style="color:var(--clr-text-secondary);font-weight:400;font-size:0.8rem;"
            >— SFTP</span
          >
        </div>
        <div class="mt-2 flex flex-col gap-2">
          <input
            class="input w-full"
            placeholder="user@host"
            bind:value={syncRemoteHost}
            on:blur={async () => {
              try {
                await setSyncDetails(
                  syncRemoteHost.trim(),
                  syncRemotePath.trim(),
                );
              } catch (e) {
                toastStore.error(`Sync: ${String(e)}`);
              }
            }}
          /><input
            class="input w-full"
            placeholder="/remote/path"
            bind:value={syncRemotePath}
            on:blur={async () => {
              try {
                await setSyncDetails(
                  syncRemoteHost.trim(),
                  syncRemotePath.trim(),
                );
              } catch (e) {
                toastStore.error(`Sync: ${String(e)}`);
              }
            }}
          />
        </div>
        <div class="flex items-center justify-between mt-3">
          <label
            class="flex items-center gap-2 text-sm cursor-pointer"
            style="color:var(--clr-text-secondary);"
            ><span class="gale-switch"
              ><input type="checkbox" bind:checked={syncVerbose} /><span
                class="gale-switch-track"
              ></span></span
            >Verbose log</label
          >
          <div class="flex gap-2">
            <button
              class="btn btn-sm"
              disabled={$syncLogStore.isBusy ||
                !syncRemoteHost.trim() ||
                !syncRemotePath.trim()}
              on:click={openSyncAuthManual}>Credentials…</button
            ><button
              class="btn btn-sm primary"
              disabled={$syncLogStore.isBusy ||
                !syncRemoteHost.trim() ||
                !syncRemotePath.trim()}
              on:click={handleSync}
              >{$syncLogStore.isBusy ? "Syncing…" : "Sync Now"}</button
            >
          </div>
        </div>
      </div>
    </div>
  </div>

  <!-- Updates -->
  <div class="prefs-group">
    <div class="prefs-group-title">App Updates</div>
    <div class="prefs-boxed-list">
      <div class="prefs-row">
        <div class="prefs-row-text">
          <div class="prefs-row-title">Updates</div>
          <div class="prefs-row-subtitle">
            {#if currentVersion}Version {currentVersion} ·
            {/if}{#if runningInFlatpak}Managed by Flatpak — flatpak update{:else if updateVersion}Update
              ready: {updateVersion}{:else}Check for updates{/if}{#if updateLastChecked && !runningInFlatpak}
              · Last checked {updateLastChecked.toLocaleString()}{/if}
          </div>
        </div>
        {#if !runningInFlatpak}<div class="prefs-row-suffix">
            <button
              class="btn btn-sm primary"
              disabled={updateCheckInProgress || updateInstallInProgress}
              on:click={updateVersion ? installAvailableUpdate : checkUpdates}
              >{#if updateCheckInProgress}Checking…{:else if updateInstallInProgress}Installing…{:else if updateVersion}Install
                Update{:else}Check for Updates{/if}</button
            >
          </div>{/if}
      </div>
    </div>
  </div>
  <ExportModpackModal
    isVisible={showExportModal}
    initialName={exportMeta.name || ""}
    initialVersion={exportMeta.version || "1.0.0"}
    initialDescription={exportMeta.description || ""}
    initialAuthor={exportMeta.author || ""}
    on:close={() => (showExportModal = false)}
    on:submit={(e) => handleExportModpack(e.detail)}
  />

  <SyncAuthModal
    isVisible={showSyncAuthModal}
    description={syncAuthPurpose === "fallback"
      ? "Auto-discovery found no usable key. Provide credentials to continue."
      : "Choose how to authenticate with the remote server. Sync Now will try your SSH keys automatically if you skip this."}
    on:close={() => (showSyncAuthModal = false)}
    on:submit={(e) => handleSyncAuthSubmit(e.detail)}
  />
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
