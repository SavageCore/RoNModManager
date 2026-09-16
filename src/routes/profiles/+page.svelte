<script lang="ts">
  import * as commands from "$lib/api/commands";
  import { toastStore } from "$lib/stores/toast";
  import type { Profile } from "$lib/types";
  import { get } from "svelte/store";
  import { onMount } from "svelte";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import {
    incognitoMode,
    DUMMY_PROFILES,
    wizardScreenshotMode,
  } from "$lib/stores/incognitoMode";
  import {
    Check,
    Copy,
    Ellipsis,
    Gamepad2,
    Monitor,
    Pencil,
    Play,
    Power,
    Trash2,
  } from "@lucide/svelte";

  $: effectiveProfiles =
    $incognitoMode && !$wizardScreenshotMode ? DUMMY_PROFILES : profiles;

  let profiles: Profile[] = [];
  let loading = false;
  let showForm = false;
  const STEAM_RUNNING_MSG =
    "Steam is still running (check tray) - quit Steam, then retry.";
  let formName = "";
  let formDescription = "";
  let formEnabledGroups: string[] = [];
  let editingProfile: Profile | null = null;

  // Overlay actions menu for one profile at a time. Rendered with
  // position:fixed against the toggle button's viewport rect, so it escapes
  // the card's overflow clipping and always paints on top.
  import { tick } from "svelte";
  let openMenu: string | null = null;
  let menuPos = { top: 0, bottom: 0, left: 0 };
  let menuFlipUp = false;
  let menuEl: HTMLDivElement | null = null;

  function toggleMenu(name: string, anchor?: HTMLElement) {
    if (openMenu === name) {
      openMenu = null;
      return;
    }
    openMenu = name;
    menuFlipUp = false;
    if (anchor) {
      const r = anchor.getBoundingClientRect();
      const width = 248;
      menuPos = {
        top: r.bottom + 6,
        bottom: window.innerHeight - r.top + 6,
        left: Math.max(
          8,
          Math.min(r.right - width, window.innerWidth - width - 8),
        ),
      };
    }
    // After paint, flip upward only if the menu would run off-viewport.
    void tick().then(() => {
      if (openMenu !== name || !menuEl) return;
      const h = menuEl.offsetHeight;
      if (menuPos.top + h > window.innerHeight - 8) {
        menuFlipUp = true;
      }
    });
  }

  function closeMenu() {
    openMenu = null;
  }

  onMount(() => {
    void loadProfiles();
    const dismiss = (event: Event) => {
      if (!openMenu) return;
      const t = event.target as HTMLElement;
      if (
        t.closest("[data-profile-menu-anchor]") ||
        t.closest("[data-profile-menu]")
      )
        return;
      openMenu = null;
    };
    document.addEventListener("mousedown", dismiss);
    window.addEventListener("scroll", closeMenu, true);
    window.addEventListener("resize", closeMenu);
    return () => {
      document.removeEventListener("mousedown", dismiss);
      window.removeEventListener("scroll", closeMenu, true);
      window.removeEventListener("resize", closeMenu);
    };
  });

  async function loadProfiles() {
    try {
      loading = true;
      if (get(incognitoMode) && !$wizardScreenshotMode) return;
      profiles = await commands.listProfiles();
      // Preload one-click shortcut status so buttons show Remove vs Create.
      for (const p of profiles) {
        void refreshShortcutStatus(p.name);
      }
    } catch (err) {
      toastStore.fromError(err);
    } finally {
      loading = false;
    }
  }

  function openForm(profile?: Profile) {
    if (profile) {
      editingProfile = profile;
      formName = profile.name;
      formDescription = profile.description || "";
      formEnabledGroups = [...profile.installed_mod_names];
    } else {
      editingProfile = null;
      formName = "";
      formDescription = "";
      formEnabledGroups = [];
    }
    showForm = true;
  }

  function closeForm() {
    showForm = false;
    editingProfile = null;
    formName = "";
    formDescription = "";
    formEnabledGroups = [];
  }

  async function handleSubmit() {
    try {
      const name = formName.trim();
      if (!name) {
        toastStore.warning("Profile name is required");
        return;
      }
      if (editingProfile) {
        // Renaming an existing profile
        await commands.renameProfile(
          editingProfile.name,
          name,
          formDescription || null,
          formEnabledGroups,
        );
        // If the renamed profile was the active one, the backend already updated the active profile
        window.dispatchEvent(
          new CustomEvent("ron:profile-changed", { detail: { name } }),
        );
      } else {
        // Creating a new profile
        await commands.saveProfile(
          name,
          formDescription || null,
          formEnabledGroups,
        );
        const profile = await commands.applyProfile(name);
        const config = await commands.getConfig();
        // When link-on-launch-only is on, creating/applying a profile is
        // staging-only; the game folder is (un)linked at launch time.
        if (config.game_path && !config.link_on_launch_only)
          await commands.syncModLinks(profile.installed_mod_names);
        window.dispatchEvent(
          new CustomEvent("ron:profile-changed", { detail: { name } }),
        );
      }
      await loadProfiles();
      closeForm();
      toastStore.success(
        editingProfile
          ? `Profile renamed: ${name}`
          : `Profile created and switched: ${name}`,
      );
    } catch (err) {
      toastStore.fromError(err);
    }
  }

  let deleteTarget: string | null = null;

  function askDeleteProfile(name: string) {
    deleteTarget = name;
  }

  async function handleDelete(name: string) {
    deleteTarget = null;
    try {
      await commands.deleteProfile(name);
      delete shortcutInfo[name];
      delete steamBlocked[name];
      if (openMenu === name) openMenu = null;
      await loadProfiles();
      toastStore.success(`Profile "${name}" deleted successfully.`);
    } catch (err) {
      toastStore.fromError(err);
    }
  }

  async function handleApply(name: string) {
    try {
      const profile = await commands.applyProfile(name);
      const config = await commands.getConfig();
      // When link-on-launch-only is on, applying a profile is staging-only; the
      // game folder is (un)linked at launch time.
      if (config.game_path && !config.link_on_launch_only)
        await commands.syncModLinks(profile.installed_mod_names);
      window.dispatchEvent(
        new CustomEvent("ron:profile-changed", { detail: { name } }),
      );
      toastStore.success(
        `Applied profile: ${name} (${profile.installed_mod_names.length} mod group${profile.installed_mod_names.length === 1 ? "" : "s"} enabled)`,
      );
    } catch (err) {
      toastStore.fromError(err);
    }
  }

  let shortcutBusy: string | null = null;
  let quitBusy: string | null = null;
  let shortcutInfo: Record<string, { desktop: boolean; steam: boolean }> = {};
  let steamBlocked: Record<string, boolean> = {};

  async function handleQuitSteam(name: string, autoRetry: boolean) {
    // Separate busy flag so the 16s shutdown poll never disables the
    // shortcut buttons - only the Quit button itself is disabled.
    quitBusy = name;
    try {
      const msg = await commands.quitSteam();
      toastStore.info(msg);
      if (!(await commands.isSteamRunning())) {
        // Steam was already stopped (or quit instantly): proceed directly.
        steamBlocked[name] = false;
        if (autoRetry) await handleSteamShortcut(name);
        return;
      }
      // Poll until Steam exits, then clear the blocked flag.
      for (let i = 0; i < 8; i++) {
        await new Promise((r) => setTimeout(r, 2000));
        if (!(await commands.isSteamRunning())) {
          steamBlocked[name] = false;
          toastStore.success("Steam closed.");
          if (autoRetry) await handleSteamShortcut(name);
          return;
        }
      }
      toastStore.warning(
        "Steam is still running - quit it manually (check tray).",
      );
    } catch (err) {
      toastStore.fromError(err);
    } finally {
      quitBusy = null;
    }
  }

  async function refreshShortcutStatus(name: string) {
    try {
      const s = await commands.profileShortcutStatus(name);
      shortcutInfo[name] = { desktop: !!s.desktopPath, steam: s.inSteam };
    } catch {
      // ignore - status is best-effort
    }
  }

  async function handleDesktopShortcut(name: string) {
    const has = shortcutInfo[name]?.desktop;
    shortcutBusy = name;
    try {
      if (has) {
        await commands.removeProfileShortcut(name);
        toastStore.success(`Desktop shortcut removed for "${name}".`);
      } else {
        // desktopCopy=true: launcher entry in ~/.local/share/applications AND
        // a visible copy on ~/Desktop (KDE/XFCE show it; GNOME hides ~/Desktop
        // by default, where the start-menu entry still works).
        const path = await commands.createProfileShortcut(name, false, true);
        toastStore.success(`Desktop shortcut created: ${path}`);
      }
      await refreshShortcutStatus(name);
    } catch (err) {
      // Flatpak sandbox fallback: offer the .desktop content for manual save.
      try {
        const s = await commands.profileShortcutStatus(name);
        if (s.desktopContent) {
          const blob = new Blob([s.desktopContent], { type: "text/plain" });
          const a = document.createElement("a");
          a.href = URL.createObjectURL(blob);
          a.download = `ronmm-${name.toLowerCase().replace(/[^a-z0-9]+/g, "-")}.desktop`;
          a.click();
          setTimeout(() => URL.revokeObjectURL(a.href), 5000);
          toastStore.success(
            "Sandbox blocked direct install - .desktop file downloaded. Copy it to ~/.local/share/applications/ on the host.",
          );
          return;
        }
      } catch {
        // fall through to error toast below
      }
      toastStore.fromError(err);
    } finally {
      shortcutBusy = null;
    }
  }

  async function handleSteamShortcut(name: string) {
    const has = shortcutInfo[name]?.steam;
    shortcutBusy = name;
    try {
      if (has) {
        await commands.removeProfileFromSteam(name);
        toastStore.success(
          `Removed "${name}" from Steam library. Restart Steam.`,
        );
      } else {
        if (await commands.isSteamRunning()) {
          steamBlocked[name] = true;
          toastStore.warning(STEAM_RUNNING_MSG, undefined, {
            label: "Quit Steam",
            handler: () => void handleQuitSteam(name, true),
          });
          return;
        }
        const updated = await commands.addProfileToSteam(name, false);
        toastStore.success(
          `Added "RoN - ${name}" to ${updated.length} Steam profile(s). Restart Steam to see it.`,
        );
      }
      await refreshShortcutStatus(name);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      if (msg.includes("still running")) {
        steamBlocked[name] = true;
        toastStore.warning(msg, undefined, {
          label: "Quit Steam",
          handler: () => void handleQuitSteam(name, true),
        });
      } else {
        toastStore.fromError(err);
      }
    } finally {
      shortcutBusy = null;
    }
  }

  async function handleDuplicate(name: string) {
    let newName = `${name} Copy`;
    let i = 2;
    while (profiles.some((p) => p.name === newName))
      newName = `${name} Copy ${i++}`;
    const input = prompt(`Duplicate "${name}" as:`, newName);
    if (input === null) return;
    const trimmed = input.trim();
    if (!trimmed) {
      toastStore.warning("Profile name is required");
      return;
    }
    try {
      await commands.duplicateProfile(name, trimmed);
      await loadProfiles();
      toastStore.success(`Profile duplicated: ${trimmed}`);
    } catch (err) {
      toastStore.fromError(err);
    }
  }
</script>

<section class="prefs-page">
  <h1 style="color: var(--clr-text);" class="text-2xl font-bold">
    Mod Profiles
  </h1>
  <p style="color: var(--clr-text-secondary);" class="text-sm mt-1">
    Save and load different installed-mod configurations. Profiles share one mod
    store, so switching does not duplicate data.
  </p>

  {#if showForm}
    <div class="prefs-group">
      <div class="prefs-group-title">
        {editingProfile ? "Edit Profile" : "Create Profile"}
      </div>
      <div class="prefs-boxed-list">
        <div class="prefs-row">
          <div class="prefs-row-text">
            <div class="prefs-row-title">Name</div>
          </div>
          <div class="prefs-row-suffix" style="flex:1; max-width: 280px;">
            <input
              id="profile-name"
              type="text"
              bind:value={formName}
              class="input"
              placeholder="Profile name"
            />
          </div>
        </div>
        <div class="prefs-row">
          <div class="prefs-row-text">
            <div class="prefs-row-title">Description</div>
            <div class="prefs-row-subtitle">Optional</div>
          </div>
          <div class="prefs-row-suffix" style="flex:1; max-width: 280px;">
            <textarea
              id="profile-description"
              bind:value={formDescription}
              class="textarea"
              placeholder="Profile description"
              rows="2"></textarea>
          </div>
        </div>
        <div class="prefs-row">
          <div class="prefs-row-text">
            <div class="prefs-row-subtitle">
              {editingProfile ? "" : "New profile will be applied immediately"}
            </div>
          </div>
          <div class="prefs-row-suffix">
            <button on:click={handleSubmit} class="btn primary btn-sm"
              >Save</button
            >
            <button on:click={closeForm} class="btn btn-sm">Cancel</button>
          </div>
        </div>
      </div>
    </div>
  {/if}

  <div class="prefs-group">
    <div class="prefs-group-title">Profiles</div>
    <div class="prefs-group-desc">
      {effectiveProfiles.length} profile{effectiveProfiles.length === 1
        ? ""
        : "s"}{#if !showForm}
        {" "} - create and switch between configurations{/if}
    </div>
    <div class="prefs-boxed-list">
      {#if loading && !$incognitoMode && !$wizardScreenshotMode}
        <div class="prefs-row">
          <span class="prefs-row-subtitle">Loading profiles…</span>
        </div>
      {:else if effectiveProfiles.length === 0}
        <div class="prefs-row">
          <span class="prefs-row-subtitle"
            >No profiles yet. Create one to get started.</span
          >
        </div>
      {:else}
        {#each effectiveProfiles as profile (profile.name)}
          <div class="prefs-row">
            <div class="prefs-row-text">
              <div class="prefs-row-title">{profile.name}</div>
              {#if profile.description}<div class="prefs-row-subtitle">
                  {profile.description}
                </div>{/if}
              <div class="prefs-row-subtitle">
                {new Date(profile.created_at).toLocaleDateString()} · {profile
                  .installed_mod_names.length} mod group{profile
                  .installed_mod_names.length === 1
                  ? ""
                  : "s"}
              </div>
            </div>
            <div class="prefs-row-suffix">
              <button
                on:click={() => handleApply(profile.name)}
                class="btn btn-sm primary"
              >
                <span class="btn-icon"><Play size={14} /></span>Apply</button
              >
              <button
                data-profile-menu-anchor
                on:click={(e) =>
                  toggleMenu(profile.name, e.currentTarget as HTMLElement)}
                class="btn btn-sm"
                title="More actions"
                aria-label={`More actions for ${profile.name}`}
                aria-haspopup="menu"
                aria-expanded={openMenu === profile.name}
              >
                <Ellipsis size={16} />
              </button>
            </div>
          </div>
        {/each}
      {/if}
    </div>
    {#if !showForm}
      <div style="margin-top: 0.75rem;">
        <button on:click={() => openForm()} class="btn primary btn-sm"
          >+ Create New Profile</button
        >
      </div>
    {/if}
  </div>

  {#if openMenu}
    {@const menuProfile = profiles.find((p) => p.name === openMenu)}
    {#if menuProfile}
      <div
        bind:this={menuEl}
        data-profile-menu
        class="profile-menu"
        style="top: {menuFlipUp
          ? 'auto'
          : `${menuPos.top}px`}; bottom: {menuFlipUp
          ? `${menuPos.bottom}px`
          : 'auto'}; left: {menuPos.left}px;"
        role="menu"
        aria-label={`Actions for ${menuProfile.name}`}
      >
        <button
          role="menuitem"
          class="profile-menu-item"
          on:click={() => {
            closeMenu();
            openForm(menuProfile);
          }}
        >
          <Pencil size={15} />
          <span>Edit</span>
        </button>
        <button
          role="menuitem"
          class="profile-menu-item"
          on:click={() => {
            closeMenu();
            void handleDuplicate(menuProfile.name);
          }}
        >
          <Copy size={15} />
          <span>Duplicate</span>
        </button>
        <div class="profile-menu-divider"></div>
        <button
          role="menuitem"
          class="profile-menu-item"
          disabled={shortcutBusy === menuProfile.name}
          title={shortcutInfo[menuProfile.name]?.desktop
            ? "Remove desktop shortcut"
            : "Create one-click desktop shortcut for this profile"}
          on:click={() => {
            const n = menuProfile.name;
            closeMenu();
            void refreshShortcutStatus(n);
            void handleDesktopShortcut(n);
          }}
        >
          <Monitor size={15} />
          <span
            >{shortcutInfo[menuProfile.name]?.desktop
              ? "Remove desktop shortcut"
              : "Create desktop shortcut"}</span
          >
          {#if shortcutInfo[menuProfile.name]?.desktop}
            <span class="menu-check"><Check size={14} /></span>
          {/if}
        </button>
        <button
          role="menuitem"
          class="profile-menu-item"
          disabled={shortcutBusy === menuProfile.name}
          title="Add as non-Steam game so it can be launched from Steam"
          on:click={() => {
            const n = menuProfile.name;
            closeMenu();
            void refreshShortcutStatus(n);
            void handleSteamShortcut(n);
          }}
        >
          <Gamepad2 size={15} />
          <span
            >{shortcutInfo[menuProfile.name]?.steam
              ? "Remove from Steam"
              : "Add to Steam"}</span
          >
          {#if shortcutInfo[menuProfile.name]?.steam}
            <span class="menu-check"><Check size={14} /></span>
          {/if}
        </button>
        {#if steamBlocked[menuProfile.name]}
          <button
            role="menuitem"
            class="profile-menu-item"
            disabled={quitBusy === menuProfile.name}
            title="Ask Steam to quit, then retry automatically"
            on:click={() => {
              const n = menuProfile.name;
              closeMenu();
              void handleQuitSteam(n, true);
            }}
          >
            <Power size={15} />
            <span
              >{quitBusy === menuProfile.name
                ? "Quitting Steam…"
                : "Quit Steam"}</span
            >
          </button>
        {/if}
        <div class="profile-menu-divider"></div>
        <button
          role="menuitem"
          class="profile-menu-item danger"
          on:click={() => {
            const n = menuProfile.name;
            closeMenu();
            askDeleteProfile(n);
          }}
        >
          <Trash2 size={15} />
          <span>Delete</span>
        </button>
      </div>
    {/if}
  {/if}

  <ModalShell
    isVisible={deleteTarget !== null}
    title="Delete profile?"
    width="w-[28rem]"
    on:close={() => (deleteTarget = null)}
  >
    <p style="color: var(--clr-text-secondary);" class="text-sm">
      Delete profile <strong style="color: var(--clr-text);"
        >"{deleteTarget}"</strong
      >? Its one-click desktop shortcut and Steam entry will be removed too.
      Installed mods are kept.
    </p>
    <div class="flex justify-end gap-2 mt-5">
      <button class="btn btn-sm" on:click={() => (deleteTarget = null)}
        >Cancel</button
      >
      <button
        class="btn btn-sm danger"
        on:click={() => deleteTarget && void handleDelete(deleteTarget)}
        >Delete</button
      >
    </div>
  </ModalShell>
</section>

<style>
  /* Fixed overlay: escapes the card's overflow clipping and paints on top.
     Position is set from the toggle button's viewport rect at open time. */
  .profile-menu {
    position: fixed;
    z-index: 200;
    width: 248px;
    padding: 4px;
    border-radius: 10px;
    background: var(--clr-surface, #fff);
    border: 1px solid var(--adw-border-color, #ccc);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
  }
  .btn-icon {
    display: inline-flex;
    margin-right: 0.35rem;
    vertical-align: -2px;
  }
  .profile-menu-item {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    width: 100%;
    padding: 0.5rem 0.65rem;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--clr-text);
    font-size: 0.85rem;
    cursor: pointer;
    text-align: left;
  }
  .profile-menu-item:hover:not(:disabled) {
    background: var(--clr-btn-adaptive-pressed, rgba(0, 0, 0, 0.08));
  }
  .profile-menu-item:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .profile-menu-item.danger {
    color: var(--clr-danger-300, #f44336);
  }
  .profile-menu-divider {
    height: 1px;
    margin: 4px 6px;
    background: var(--adw-border-color, #ccc);
    opacity: 0.6;
  }
  .menu-check {
    margin-left: auto;
    display: inline-flex;
    color: #4caf50;
  }
</style>
