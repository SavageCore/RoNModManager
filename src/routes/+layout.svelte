<script lang="ts">
  import { onMount } from "svelte";
  import "../app.css";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { listen } from "@tauri-apps/api/event";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import {
    LogicalPosition,
    LogicalSize,
    getCurrentWindow,
  } from "@tauri-apps/api/window";
  import {
    getConfig,
    listProfiles,
    applyProfile,
    setWindowTitle,
    saveWindowState,
    launchGameWithGroups,
    getInstalledModGroups,
    syncModLinks,
    detectGamePath,
    setGamePath,
  } from "$lib/api/commands";
  import { initTheme } from "$lib/theme";
  import type { Profile, ModProgressEvent } from "$lib/types";
  import { modToggleState } from "$lib/stores/modState";
  import { operationStatusStore } from "$lib/stores/operationStatus";
  import { Layers, Package, Play, Settings, User } from "lucide-svelte";
  import FooterStatusBar from "$lib/components/FooterStatusBar.svelte";
  import Toast from "$lib/components/Toast.svelte";
  import { tokenStore } from "$lib/stores/token";

  const APP_NAME = "Mod Manager";

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
    } catch (error) {
      console.error("Failed to launch game:", error);
      alert(`Failed to launch game: ${String(error)}`);
    } finally {
      isLaunching = false;
    }
  }

  onMount(() => {
    const unsubscribe = tokenStore.subscribe((val) => {
      hasSavedToken = val;
    });

    let cleanup = () => {};
    let unlistenProgress: UnlistenFn | null = null;
    let unlistenResize: (() => void) | null = null;
    let unlistenMove: (() => void) | null = null;
    const appWindow = getCurrentWindow();

    let resizeDebounce: ReturnType<typeof setTimeout> | null = null;
    let moveDebounce: ReturnType<typeof setTimeout> | null = null;

    const persistCurrentWindowState = async () => {
      try {
        const size = await appWindow.innerSize();
        let x: number | undefined;
        let y: number | undefined;

        try {
          const position = await appWindow.outerPosition();
          x = position.x;
          y = position.y;
        } catch {
          // Wayland may not expose reliable window position.
        }

        await saveWindowState(size.width, size.height, x, y);
      } catch {
        // Non-fatal; window state persistence should never block app usage.
      }
    };

    const handleAppFocus = () => {
      void refreshShellConfigState();
    };

    const handleVisibilityChange = () => {
      if (document.visibilityState === "visible") {
        void refreshShellConfigState();
      }
    };

    const handleProfileChanged = () => {
      void loadProfiles();
      void refreshShellConfigState();
    };

    window.addEventListener("focus", handleAppFocus);
    document.addEventListener("visibilitychange", handleVisibilityChange);
    window.addEventListener("ron:profile-changed", handleProfileChanged);

    void Promise.all([
      getConfig().then(async (config) => {
        hasGamePath = config.game_path != null;
        hasSavedToken = Boolean(config.oauth_token?.trim());

        cleanup = initTheme(config.theme);

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

        if (config.window_width && config.window_height) {
          try {
            await appWindow.setSize(
              new LogicalSize(config.window_width, config.window_height),
            );
          } catch {
            // Ignore if current platform rejects programmatic resize.
          }
        }

        if (config.window_x != null && config.window_y != null) {
          try {
            await appWindow.setPosition(
              new LogicalPosition(config.window_x, config.window_y),
            );
          } catch {
            // Wayland often rejects or virtualizes window position APIs.
          }
        }
      }),
      loadProfiles(),
    ]).catch(() => {
      cleanup = initTheme("system");
    });

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

    void listen<ModProgressEvent>("install_progress", (event) => {
      operationStatusStore.updateFromProgress(event.payload);
    }).then((fn) => {
      unlistenProgress = fn;
    });

    return () => {
      cleanup();
      if (resizeDebounce) {
        clearTimeout(resizeDebounce);
      }
      if (moveDebounce) {
        clearTimeout(moveDebounce);
      }
      if (unlistenProgress) {
        unlistenProgress();
      }
      if (unlistenResize) {
        unlistenResize();
      }
      if (unlistenMove) {
        unlistenMove();
      }
      window.removeEventListener("focus", handleAppFocus);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
      window.removeEventListener("ron:profile-changed", handleProfileChanged);
    };
  });
</script>

<div class="flex h-screen w-full flex-col">
  <!-- Global Toast Notifications -->
  <Toast />

  <!-- Gale-style HeaderBar -->
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
      <!-- Profile dropdown -->
      <div
        style="background: var(--clr-btn); color: var(--clr-text);"
        class="flex items-center gap-2 rounded-lg px-3 py-1.5 text-sm"
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
        class="btn primary btn-sm"
        on:click={() => {
          void launchWithProfile();
        }}
        disabled={!hasGamePath || isLaunching}
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

  <FooterStatusBar />
</div>
