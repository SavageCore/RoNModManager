<script lang="ts">
  import { afterNavigate, goto } from "$app/navigation";
  import { page } from "$app/stores";
  import {
    applyProfile,
    checkForUpdate,
    detectGamePath,
    fetchModpackJson,
    getConfig,
    launchGameWithGroups,
    listProfiles,
    isScreenshotMode,
    screenshotTheme,
    manageWindowGeometry,
    refreshModMetadata,
    saveWindowState,
    setGamePath,
    setWindowTitle,
    updateConfig,
  } from "$lib/api/commands";
  import FooterStatusBar from "$lib/components/FooterStatusBar.svelte";
  import ImportLogPanel from "$lib/components/ImportLogPanel.svelte";
  import SyncPanel from "$lib/components/SyncPanel.svelte";
  import Toast from "$lib/components/Toast.svelte";
  import { operationStatusStore } from "$lib/stores/operationStatus";
  import { pendingInstallUrl } from "$lib/stores/pendingInstall";
  import { toastStore } from "$lib/stores/toast";
  import { tokenStore } from "$lib/stores/token";
  import { updateCheckStore } from "$lib/stores/updateCheck";
  import { initTheme } from "$lib/theme";
  import type {
    CloseAction,
    MinimizeTarget,
    ModProgressEvent,
    OnGameLaunchAction,
    Profile,
    ThemeMode,
  } from "$lib/types";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { listen } from "@tauri-apps/api/event";
  import { onOpenUrl } from "@tauri-apps/plugin-deep-link";
  import {
    LogicalPosition,
    LogicalSize,
    availableMonitors,
    currentMonitor,
    getCurrentWindow,
    primaryMonitor,
  } from "@tauri-apps/api/window";
  import {
    Layers,
    Package,
    Play,
    RefreshCw,
    Settings,
    User,
  } from "lucide-svelte";
  import semver from "semver";
  import { onMount } from "svelte";
  import "../app.css";

  import AddModpackPanel from "$lib/components/AddModpackPanel.svelte";
  import ConfirmModal from "$lib/components/ConfirmModal.svelte";
  import InfoPanel from "$lib/components/InfoPanel.svelte";
  import SetupWizard from "$lib/components/SetupWizard.svelte";
  import { addModpackPanelStore } from "$lib/stores/addModpackPanelStore";
  import { importLogStore } from "$lib/stores/importLogStore";
  import { infoLogStore } from "$lib/stores/infoLogStore";
  import { incognitoMode, screenshotMode } from "$lib/stores/incognitoMode";

  const APP_NAME = "RoN Mod Manager";
  const UPDATE_AUTO_CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000;

  const nav = [
    { href: "/mods", label: "Mods", icon: Package },
    { href: "/collections", label: "Collections", icon: Layers },
    { href: "/profiles", label: "Profiles", icon: User },
    { href: "/settings", label: "Settings", icon: Settings },
  ];

  let selectedProfile = "Default";
  let profiles: Profile[] = [];
  let hasGamePath = false;
  let hasSavedToken = false;
  let isApplyingProfile = false;
  let isLaunching = false;
  let isRefreshingMetadata = false;
  let modpackCurrentVersion: string | null = null;
  let modpackNewVersion: string | null = null;
  let updateAvailable = false;
  let onGameLaunch: OnGameLaunchAction = "nothing";
  let closeAction: CloseAction = "quit";
  let minimizeTarget: MinimizeTarget = "taskbar";
  let askedClosePreference = false;
  let showClosePreferenceDialog = false;
  let showCloseWhileRunningDialog = false;
  let showSetupWizard = false;
  let closingFromLaunch = false;
  let forceClose = false;

  function resolveSelectedProfile(
    activeProfile: string | null | undefined,
  ): string {
    const trimmedActive = activeProfile?.trim();
    if (
      trimmedActive &&
      profiles.some((profile) => profile.name === trimmedActive)
    ) {
      return trimmedActive;
    }
    if (profiles.some((profile) => profile.name === "Default")) {
      return "Default";
    }
    return profiles[0]?.name ?? "Default";
  }

  async function refreshShellConfigState() {
    try {
      const config = await getConfig();
      hasGamePath = config.game_path != null;
      hasSavedToken = Boolean(config.oauth_token?.trim());
      tokenStore.set(hasSavedToken);
      selectedProfile = resolveSelectedProfile(config.active_profile);
      onGameLaunch = config.on_game_launch ?? "nothing";
      closeAction = config.close_action ?? "quit";
      minimizeTarget = config.minimize_target ?? "taskbar";
      askedClosePreference = config.asked_close_preference ?? false;
      await updateWindowTitle();
    } catch {
      // Non-fatal: shell can continue using previous state.
    }
  }

  async function handleProfileChange() {
    if (!selectedProfile) return;
    try {
      isApplyingProfile = true;
      await applyProfile(selectedProfile);
      await updateWindowTitle();
    } catch (error) {
      console.error("Failed to apply profile:", error);
    } finally {
      isApplyingProfile = false;
    }
  }

  async function updateWindowTitle() {
    await setWindowTitle(`${selectedProfile} | ${APP_NAME}`);
  }

  async function loadProfiles() {
    try {
      const list = await listProfiles().catch(() => []);
      profiles = list;
      const config = await getConfig();
      selectedProfile = resolveSelectedProfile(config.active_profile);
      await updateWindowTitle();
    } catch (error) {
      console.error("Failed to load profiles:", error);
    }
  }

  async function doMinimize() {
    const appWindow = getCurrentWindow();
    if (minimizeTarget === "tray") {
      await appWindow.hide();
    } else {
      await appWindow.minimize();
    }
  }

  async function launchWithProfile() {
    if (!hasGamePath) {
      alert("Game path is not configured. Open Settings first.");
      return;
    }

    try {
      isLaunching = true;
      if (!selectedProfile) {
        return;
      }

      const profile = await applyProfile(selectedProfile);
      await launchGameWithGroups(profile.installed_mod_names);

      if (onGameLaunch === "minimize") {
        await doMinimize();
      } else if (onGameLaunch === "close") {
        window.dispatchEvent(new CustomEvent("ron:launch-close"));
        await getCurrentWindow().close();
      }
    } catch (error) {
      console.error("Failed to launch game:", error);
      alert(`Failed to launch game: ${String(error)}`);
    } finally {
      isLaunching = false;
    }
  }

  async function handleRefreshMetadata() {
    try {
      isRefreshingMetadata = true;
      infoLogStore.start();
      infoLogStore.addLine("Refreshing mod metadata from source links...");
      const result = await refreshModMetadata();
      const tone = result.failed > 0 ? "error" : "success";
      infoLogStore.addLine(
        `Metadata refresh complete: checked ${result.checked}, refreshed ${result.refreshed}, skipped ${result.skipped}, failed ${result.failed}.`,
      );
      infoLogStore.finish(tone);
      window.dispatchEvent(new CustomEvent("ron:metadata-refreshed"));
    } catch (error) {
      console.error("Failed to refresh mod metadata:", error);
      infoLogStore.addLine(`Metadata refresh failed: ${String(error)}`);
      infoLogStore.finish("error");
    } finally {
      isRefreshingMetadata = false;
    }
  }

  async function handleWizardDismiss() {
    showSetupWizard = false;
    await refreshShellConfigState();
  }

  async function handleClosePreference(
    action: CloseAction,
    target: MinimizeTarget,
  ) {
    showClosePreferenceDialog = false;
    closeAction = action;
    minimizeTarget = target;
    askedClosePreference = true;
    await updateConfig({
      close_action: action,
      minimize_target: target,
      asked_close_preference: true,
    });
    if (action === "minimize") {
      await doMinimize();
    } else {
      await getCurrentWindow().close();
    }
  }

  async function handleForceClose() {
    forceClose = true;
    await getCurrentWindow().close();
  }

  afterNavigate(() => {
    void refreshShellConfigState();
  });

  onMount(() => {
    const unsubscribe = tokenStore.subscribe((val) => {
      hasSavedToken = val;
    });

    let cleanup = () => {};
    let unlistenFunctions: UnlistenFn[] = [];
    let unlistenResize: (() => void) | null = null;
    let unlistenMove: (() => void) | null = null;
    const appWindow = getCurrentWindow();

    let resizeDebounce: ReturnType<typeof setTimeout> | null = null;
    let moveDebounce: ReturnType<typeof setTimeout> | null = null;

    // On native Wayland the compositor owns window geometry - it centres and
    // sizes windows sensibly and forbids apps setting an absolute position. We
    // leave it entirely alone there and only persist/restore size and position
    // on X11/XWayland, where we can reliably control them.
    // In screenshot mode we also skip geometry management so the window starts
    // at the default size defined in tauri.conf.json every time.
    const screenshotModePromise = isScreenshotMode().catch(() => false);
    const manageGeometry = screenshotModePromise.then((sm) =>
      sm ? false : manageWindowGeometry().catch(() => false),
    );

    // In screenshot mode: auto-activate incognito and wire up number-key
    // page navigation (1=Mods 2=Collections 3=Profiles 4=Settings) so the
    // script can drive the app without relying on coordinate-based clicks.
    void screenshotModePromise.then((isScreenshot) => {
      if (!isScreenshot) return;
      screenshotMode.set(true);
      incognitoMode.set(true);
      const ssPages = ["/mods", "/collections", "/profiles", "/settings"];
      window.addEventListener("keydown", (e) => {
        const idx = Number(e.key) - 1;
        if (idx >= 0 && idx < ssPages.length) void goto(ssPages[idx]);
      });
    });

    const persistCurrentWindowState = async () => {
      if (!(await manageGeometry)) {
        return;
      }
      try {
        // innerSize()/outerPosition() return physical pixels, but we restore
        // via LogicalSize/LogicalPosition. Convert to logical here so the saved
        // values match the units used on restore - otherwise the window grows
        // by the scale factor on every launch under fractional scaling.
        const scale = await appWindow.scaleFactor();
        const size = (await appWindow.innerSize()).toLogical(scale);
        const position = (await appWindow.outerPosition()).toLogical(scale);
        await saveWindowState(size.width, size.height, position.x, position.y);
      } catch {
        // Non-fatal; window state persistence should never block app usage.
      }
    };

    const handleAppFocus = () => {
      void refreshShellConfigState();
    };

    void onOpenUrl((urls) => {
      for (const url of urls) {
        if (url.startsWith("ronmm://install/nexus/")) {
          const id = url
            .replace("ronmm://install/nexus/", "")
            .replace(/\/$/, "");
          pendingInstallUrl.set(
            `https://www.nexusmods.com/readyornot/mods/${id}`,
          );
          void goto("/mods");
        } else if (url.startsWith("ronmm://install/modio/")) {
          const id = url
            .replace("ronmm://install/modio/", "")
            .replace(/\/$/, "");
          pendingInstallUrl.set(id);
          void goto("/mods");
        } else if (url.startsWith("ronmm://modpack/")) {
          const urlStr = url.replace("ronmm://modpack/", "");
          addModpackPanelStore.open("add", { url: urlStr });
        }
      }
    });

    const handleVisibilityChange = () => {
      if (document.visibilityState === "visible") {
        void refreshShellConfigState();
      }
    };

    const handleProfileChanged = () => {
      void loadProfiles();
      void refreshShellConfigState();
    };

    const handleLaunchClose = () => {
      closingFromLaunch = true;
    };

    const handleIncognitoKeybind = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "i") {
        e.preventDefault();
        incognitoMode.update((v) => !v);
      }
    };

    window.addEventListener("focus", handleAppFocus);
    document.addEventListener("visibilitychange", handleVisibilityChange);
    window.addEventListener("ron:profile-changed", handleProfileChanged);
    window.addEventListener("ron:launch-close", handleLaunchClose);
    window.addEventListener("keydown", handleIncognitoKeybind);

    void Promise.all([
      getConfig().then(async (config) => {
        hasGamePath = config.game_path != null;
        hasSavedToken = Boolean(config.oauth_token?.trim());

        const themeOverride = await screenshotModePromise.then((sm) =>
          sm ? screenshotTheme().catch(() => null) : null,
        );
        cleanup = initTheme((themeOverride as ThemeMode) ?? config.theme);

        // Modpack update check
        if (config.modpack_url && config.modpack_version) {
          try {
            const remote = await fetchModpackJson(config.modpack_url);
            if (
              remote &&
              remote.version &&
              semver.valid(remote.version) &&
              semver.valid(config.modpack_version) &&
              semver.gt(remote.version, config.modpack_version)
            ) {
              console.log("Update available now");
              modpackCurrentVersion = config.modpack_version;
              modpackNewVersion = remote.version;
              updateAvailable = true;
              addModpackPanelStore.open("update", {
                currentVersion: modpackCurrentVersion,
                newVersion: modpackNewVersion,
              });
            }
          } catch (e) {
            console.error("Failed to check for modpack update:", e);
          }
        }

        if (!hasGamePath) {
          try {
            const detectedPath = await detectGamePath();
            if (detectedPath) {
              await setGamePath(detectedPath);
              hasGamePath = true;
            }
          } catch {
            // Non-fatal: user can set game path manually in Settings.
          }
        }

        if (await manageGeometry) {
          try {
            // Clamp restored geometry so a stale or oversized saved value can
            // never leave the window larger than the usable screen.
            // minWidth/minHeight mirror tauri.conf.json.
            const MIN_WIDTH = 1024;
            const MIN_HEIGHT = 720;
            const MARGIN = 16; // breathing room / padding
            // Reserve for an edge panel the compositor doesn't report in workArea
            // (e.g. KWin on Wayland hands back the full screen as the work area).
            const PANEL_RESERVE = 56;

            // currentMonitor() returns null on native Wayland, so fall back to
            // the primary monitor and finally any available monitor.
            const monitor =
              (await currentMonitor()) ??
              (await primaryMonitor()) ??
              (await availableMonitors())[0] ??
              null;

            let maxWidth = Infinity;
            let maxHeight = Infinity;
            if (monitor) {
              const sf = monitor.scaleFactor;
              // setSize sets the *inner* size, but the window's on-screen
              // footprint is the outer size (title bar + borders). Subtract that
              // delta or the visible window overflows the screen.
              const inner = (await appWindow.innerSize()).toLogical(sf);
              const outer = (await appWindow.outerSize()).toLogical(sf);
              const decoW = Math.max(0, outer.width - inner.width);
              const decoH = Math.max(0, outer.height - inner.height);

              const fullW = monitor.size.width / sf;
              const fullH = monitor.size.height / sf;
              let availW = monitor.workArea.size.width / sf;
              let availH = monitor.workArea.size.height / sf;
              // If workArea wasn't shrunk versus the full resolution, the
              // compositor isn't subtracting panels - reserve the space ourselves.
              if (availH >= fullH) availH -= PANEL_RESERVE;

              maxWidth = availW - decoW - MARGIN;
              maxHeight = availH - decoH - MARGIN;
            }

            if (config.window_width && config.window_height) {
              const width = Math.max(
                MIN_WIDTH,
                Math.min(config.window_width, maxWidth),
              );
              const height = Math.max(
                MIN_HEIGHT,
                Math.min(config.window_height, maxHeight),
              );
              try {
                await appWindow.setSize(new LogicalSize(width, height));
              } catch {
                // Ignore if current platform rejects programmatic resize.
              }

              if (config.window_x != null && config.window_y != null) {
                const x =
                  maxWidth === Infinity
                    ? config.window_x
                    : Math.max(0, Math.min(config.window_x, maxWidth - width));
                const y =
                  maxHeight === Infinity
                    ? config.window_y
                    : Math.max(
                        0,
                        Math.min(config.window_y, maxHeight - height),
                      );
                try {
                  await appWindow.setPosition(new LogicalPosition(x, y));
                } catch {
                  // Position APIs can still fail; non-fatal.
                }
              }
            }
          } catch {
            // Monitor query failed; leave the window at its default geometry.
          }
        }

        onGameLaunch = config.on_game_launch ?? "nothing";
        closeAction = config.close_action ?? "quit";
        minimizeTarget = config.minimize_target ?? "taskbar";
        askedClosePreference = config.asked_close_preference ?? false;

        if (!config.setup_wizard_complete && !(await screenshotModePromise)) {
          const hasAnyKey =
            Boolean(config.modio_api_key?.trim()) ||
            Boolean(config.oauth_token?.trim()) ||
            Boolean(config.nexus_api_key?.trim());
          if (hasAnyKey) {
            void updateConfig({ setup_wizard_complete: true });
          } else {
            showSetupWizard = true;
          }
        }
      }),
      loadProfiles(),
    ]).catch(() => {
      cleanup = initTheme("system");
    });

    void (async () => {
      const lastCheckedAt = $updateCheckStore;
      if (
        lastCheckedAt &&
        Date.now() - lastCheckedAt < UPDATE_AUTO_CHECK_INTERVAL_MS
      )
        return;
      try {
        const info = await checkForUpdate();
        updateCheckStore.markChecked();
        if (info.available) {
          toastStore.success(`App update available: v${info.version}`);
        }
      } catch {
        // Non-fatal: silently skip if update check fails on startup
      }
    })();

    void appWindow
      .onResized(() => {
        if (resizeDebounce) {
          clearTimeout(resizeDebounce);
        }
        resizeDebounce = setTimeout(() => {
          void persistCurrentWindowState();
        }, 250);
      })
      .then((fn) => {
        unlistenResize = fn;
      });

    void appWindow
      .onMoved(() => {
        if (moveDebounce) {
          clearTimeout(moveDebounce);
        }
        moveDebounce = setTimeout(() => {
          void persistCurrentWindowState();
        }, 250);
      })
      .then((fn) => {
        unlistenMove = fn;
      });

    void appWindow
      .onCloseRequested(async (event) => {
        if (closingFromLaunch || forceClose) {
          closingFromLaunch = false;
          forceClose = false;
          return;
        }
        if ($importLogStore.mods.some((m) => m.status === "running")) {
          event.preventDefault();
          showCloseWhileRunningDialog = true;
          return;
        }
        if (!askedClosePreference) {
          event.preventDefault();
          showClosePreferenceDialog = true;
        } else if (closeAction === "minimize") {
          event.preventDefault();
          await doMinimize();
        }
      })
      .then((fn) => {
        unlistenFunctions.push(fn);
      });

    void listen<ModProgressEvent>("install_progress", (event) => {
      operationStatusStore.updateFromProgress(event.payload);
    }).then((fn) => {
      unlistenFunctions.push(fn);
    });

    void listen<ModProgressEvent>("export_progress", (event) => {
      operationStatusStore.updateFromProgress(event.payload);
    }).then((fn) => {
      unlistenFunctions.push(fn);
    });

    void listen<ModProgressEvent>("sync_progress", (event) => {
      operationStatusStore.updateFromProgress(event.payload);
    }).then((fn) => {
      unlistenFunctions.push(fn);
    });

    return () => {
      cleanup();
      if (resizeDebounce) {
        clearTimeout(resizeDebounce);
      }
      if (moveDebounce) {
        clearTimeout(moveDebounce);
      }
      unlistenFunctions.forEach((fn) => fn());
      if (unlistenResize) {
        unlistenResize();
      }
      if (unlistenMove) {
        unlistenMove();
      }
      window.removeEventListener("focus", handleAppFocus);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
      window.removeEventListener("ron:profile-changed", handleProfileChanged);
      window.removeEventListener("ron:launch-close", handleLaunchClose);
      window.removeEventListener("keydown", handleIncognitoKeybind);
    };
  });
</script>

<div class="flex h-screen w-full flex-col">
  <!-- Global Toast Notifications -->
  <Toast />

  <!-- HeaderBar -->
  <header
    style="background: var(--clr-surface); border-bottom: 1px solid var(--adw-border-color); color: var(--clr-text);"
    class="sticky top-0 z-50 flex h-14 items-center justify-between px-4 shadow-sm gap-4"
  >
    <!-- Left: Logo -->
    <div class="flex items-center gap-3 min-w-0">
      <img
        src="/icon.ico"
        alt="RoN Mod Manager"
        class="h-8 w-8 rounded-lg flex-shrink-0"
        style="background: #1e1e1e; padding: 2px;"
      />
      <h1 class="text-base font-semibold truncate">RoN Mod Manager</h1>
    </div>

    <!-- Right: Action buttons (always visible) -->
    <div class="flex items-center gap-2 ml-auto">
      <button
        class="btn btn-sm h-9 w-9 px-0 flex items-center justify-center"
        on:click={() => {
          void handleRefreshMetadata();
        }}
        disabled={isRefreshingMetadata}
        title="Refresh mod metadata from Nexus and mod.io links"
        aria-label="Refresh mod metadata"
      >
        <RefreshCw
          size={16}
          class={isRefreshingMetadata ? "is-spinning" : ""}
        />
      </button>

      <!-- Profile dropdown -->
      <div
        style="background: var(--clr-btn); color: var(--clr-text);"
        class="flex h-9 items-center gap-2 rounded-lg px-3 text-sm"
      >
        <label
          for="header-profile-select"
          style="color: var(--clr-text-secondary);">Profile:</label
        >
        <select
          id="header-profile-select"
          class="bg-transparent border-none text-sm font-medium cursor-pointer"
          bind:value={selectedProfile}
          disabled={profiles.length === 0}
          on:change={handleProfileChange}
        >
          {#each profiles as profile (profile.name)}
            <option value={profile.name}>{profile.name}</option>
          {/each}
        </select>
      </div>

      <!-- Launch Game button -->
      <button
        class="btn primary btn-sm h-9"
        on:click={() => {
          void launchWithProfile();
        }}
        disabled={!hasGamePath ||
          isLaunching ||
          $importLogStore.mods.some((m) => m.status === "running")}
        title="Launch Ready or Not with selected profile"
      >
        <Play size={16} class="inline mr-1" />
        {isLaunching ? "Launching..." : "Launch Game"}
      </button>
    </div>
  </header>

  {#if !hasSavedToken}
    <div
      role="button"
      tabindex="0"
      on:click={() => {
        goto("/settings");
      }}
      on:keydown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          goto("/settings");
        }
      }}
      style="background: color-mix(in srgb, var(--clr-primary-300) 12%, var(--clr-surface)); border-bottom: 1px solid var(--adw-border-color);"
      class="px-4 py-2 text-sm flex items-center justify-between gap-3 cursor-pointer"
      title="Go to settings to set token"
    >
      <span style="color: var(--clr-text);"
        >Set your mod.io token to install mods from links and use API-backed
        features.</span
      >
      <button
        class="btn btn-sm primary"
        on:click|stopPropagation={() => {
          goto("/settings");
        }}
      >
        Set Token
      </button>
    </div>
  {/if}

  <div class="flex flex-1 overflow-hidden">
    <!-- Gale-style Sidebar -->
    <aside
      style="background: var(--clr-surface-variant); border-right: 1px solid var(--adw-border-color);"
      class="flex w-20 flex-col items-center p-0 flex-shrink-0"
    >
      <nav class="flex w-full flex-col items-center gap-1 px-2 py-4">
        {#each nav as item (item.href)}
          <a
            href={item.href}
            title={item.label}
            aria-label={item.label}
            style={$page.url.pathname === item.href
              ? `background: var(--clr-primary-300); color: var(--clr-primary-text);`
              : `color: var(--clr-text);`}
            class={`flex h-12 w-12 items-center justify-center rounded-lg transition-all hover:bg-[var(--clr-btn-adaptive-hover)] ${
              $page.url.pathname === item.href ? "shadow-sm" : ""
            }`}
          >
            <svelte:component this={item.icon} size={20} />
          </a>
        {/each}
      </nav>
    </aside>

    <!-- Main content area -->
    <main style="background: var(--clr-bg);" class="flex-1 overflow-auto p-6">
      <div class="mx-auto max-w-5xl">
        <slot />
      </div>
    </main>
  </div>

  <ImportLogPanel />
  <SyncPanel />
  <AddModpackPanel />
  <InfoPanel />
  <FooterStatusBar />

  {#if showSetupWizard}
    <SetupWizard onDismiss={handleWizardDismiss} />
  {/if}

  <ConfirmModal
    isVisible={showCloseWhileRunningDialog}
    title="Installation in progress"
    message="Mods are still being installed. Closing now may leave them in a broken state."
    confirmLabel="Close anyway"
    onConfirm={() => void handleForceClose()}
    onCancel={() => {
      showCloseWhileRunningDialog = false;
    }}
  />

  {#if showClosePreferenceDialog}
    <div
      class="fixed inset-0 bg-black/60 flex items-center justify-center z-[100]"
    >
      <div
        style="background: var(--clr-surface); border: 1px solid var(--adw-border-color);"
        class="rounded-lg shadow-2xl w-[420px] p-6"
      >
        <h2 class="text-lg font-semibold mb-2" style="color: var(--clr-text);">
          When closing the window…
        </h2>
        <p class="text-sm mb-6" style="color: var(--clr-text-secondary);">
          Choose what happens when you click the close button. You can change
          this at any time in Settings.
        </p>
        <div class="flex flex-col gap-3">
          <button
            class="btn btn-primary w-full"
            on:click={() => void handleClosePreference("quit", minimizeTarget)}
          >
            Quit the app
          </button>
          <button
            class="btn w-full"
            style="background: var(--clr-surface-variant); color: var(--clr-text);"
            on:click={() => void handleClosePreference("minimize", "taskbar")}
          >
            Minimise to taskbar
          </button>
          <button
            class="btn w-full"
            style="background: var(--clr-surface-variant); color: var(--clr-text);"
            on:click={() => void handleClosePreference("minimize", "tray")}
          >
            Minimise to system tray
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>
