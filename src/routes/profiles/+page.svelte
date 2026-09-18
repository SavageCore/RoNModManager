<script lang="ts">
  import * as commands from "$lib/api/commands";
  import { toastStore } from "$lib/stores/toast";
  import type { Profile } from "$lib/types";
  import { get } from "svelte/store";
  import { onMount } from "svelte";
  import ConfirmModal from "$lib/components/ConfirmModal.svelte";
  import { registerTourActions } from "$lib/tour/registry";
  import {
    incognitoMode,
    DUMMY_PROFILES,
    wizardScreenshotMode,
  } from "$lib/stores/incognitoMode";
  import { Ellipsis, Play } from "@lucide/svelte";
  import CustomMenu from "$lib/components/CustomMenu.svelte";
  import type { MenuItem } from "$lib/components/CustomMenu.svelte";

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

  let openMenu: string | null = null;
  let menuComponent: CustomMenu;
  let menuItems: MenuItem[] = [];

  function toggleMenu(name: string, anchor?: HTMLElement) {
    if (openMenu === name) {
      menuComponent?.close();
      openMenu = null;
      return;
    }
    const menuProfile = profiles.find((p) => p.name === name);
    if (!menuProfile || !anchor) return;
    openMenu = name;
    menuItems = buildMenuItems(menuProfile);
    menuComponent.openMenu({ anchor });
  }

  function closeMenu() {
    menuComponent?.close();
    openMenu = null;
  }

  function buildMenuItems(menuProfile: Profile): MenuItem[] {
    const items: MenuItem[] = [
      {
        id: "edit",
        label: "Edit",
        icon: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"/></svg>',
        action: () => {
          closeMenu();
          openForm(menuProfile);
        },
      },
      {
        id: "duplicate",
        label: "Duplicate",
        icon: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>',
        action: () => {
          closeMenu();
          void handleDuplicate(menuProfile.name);
        },
      },
      { id: "div1", divider: true },
      {
        id: "desktop",
        label: shortcutInfo[menuProfile.name]?.desktop
          ? "Remove desktop shortcut"
          : "Create desktop shortcut",
        icon: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="20" height="14" x="2" y="3" rx="2"/><line x1="8" x2="16" y1="21" y2="21"/><line x1="12" x2="12" y1="17" y2="21"/></svg>',
        disabled: shortcutBusy === menuProfile.name,
        check: !!shortcutInfo[menuProfile.name]?.desktop,
        action: () => {
          const n = menuProfile.name;
          closeMenu();
          void refreshShortcutStatus(n);
          void handleDesktopShortcut(n);
        },
      },
      {
        id: "steam",
        label: shortcutInfo[menuProfile.name]?.steam
          ? "Remove from Steam"
          : "Add to Steam",
        icon: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2v0a10 10 0 0 1 10 10v0a10 10 0 0 1-10 10v0a10 10 0 0 1-10-10v0a10 10 0 0 1 10-10v0Z"/><path d="M12 12m-3 0a3 3 0 1 0 6 0a3 3 0 1 0-6 0"/></svg>',
        disabled: shortcutBusy === menuProfile.name,
        check: !!shortcutInfo[menuProfile.name]?.steam,
        action: () => {
          const n = menuProfile.name;
          closeMenu();
          void refreshShortcutStatus(n);
          void handleSteamShortcut(n);
        },
      },
    ];
    if (steamBlocked[menuProfile.name]) {
      items.push({
        id: "quit-steam",
        label: quitBusy === menuProfile.name ? "Quitting Steam…" : "Quit Steam",
        icon: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18.36 6.64a9 9 0 1 1-12.73 0"/><line x1="12" x2="12" y1="2" y2="12"/></svg>',
        disabled: quitBusy === menuProfile.name,
        action: () => {
          const n = menuProfile.name;
          closeMenu();
          void handleQuitSteam(n, true);
        },
      });
    }
    items.push(
      { id: "div2", divider: true },
      {
        id: "delete",
        label: "Delete",
        icon: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/></svg>',
        danger: true,
        action: () => {
          const n = menuProfile.name;
          closeMenu();
          askDeleteProfile(n);
        },
      },
    );
    return items;
  }

  onMount(() => {
    void loadProfiles();
    unregisterTourActions = registerTourActions("profiles", {
      "open-create-form": () => openForm(),
      // Non-destructive: keeps anything already typed when a step is re-entered
      // with Back.
      "ensure-create-form": () => {
        if (!showForm) openForm();
      },
      "close-create-form": () => closeForm(),
      // Next on the form's last card saves when something has been typed and
      // backs out when the form is untouched, so either way the tour moves on.
      "submit-create-form": () => {
        if (formName.trim() || formDescription.trim()) {
          void handleSubmit();
        } else {
          closeForm();
        }
      },
    });
    return () => {
      unregisterTourActions?.();
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
  let showDeleteConfirm = false;
  let unregisterTourActions: (() => void) | null = null;

  function askDeleteProfile(name: string) {
    deleteTarget = name;
    showDeleteConfirm = true;
  }

  async function handleDelete(name: string) {
    deleteTarget = null;
    try {
      const result = await commands.deleteProfile(name);
      delete shortcutInfo[name];
      delete steamBlocked[name];
      if (openMenu === name) openMenu = null;
      await loadProfiles();
      if (result.appliedProfile) {
        // The backend applied the fallback; tell the rest of the app so the
        // mods list and header follow the new active profile.
        window.dispatchEvent(
          new CustomEvent("ron:profile-changed", {
            detail: { name: result.appliedProfile },
          }),
        );
        toastStore.success(
          `Profile "${name}" deleted. Applied ${result.appliedProfile}.`,
        );
      } else {
        toastStore.success(`Profile "${name}" deleted successfully.`);
      }
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
              data-tour="profile-name"
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
              data-tour="profile-description"
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
          <div class="prefs-row-suffix" data-tour="profile-actions">
            <button
              on:click={handleSubmit}
              class="btn primary btn-sm"
              data-tour="profile-save">Save</button
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
                data-tour="profile-apply"
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
        <button
          on:click={() => openForm()}
          class="btn primary btn-sm"
          data-tour="profiles-create">+ Create New Profile</button
        >
      </div>
    {/if}
  </div>

  <CustomMenu
    bind:this={menuComponent}
    items={menuItems}
    width={248}
    on:close={() => (openMenu = null)}
  />

  <ConfirmModal
    bind:isVisible={showDeleteConfirm}
    title="Delete profile?"
    message={`Delete profile <strong>"${deleteTarget}"</strong>? Its one-click desktop shortcut and Steam entry will be removed too. Installed mods are kept.`}
    confirmLabel="Delete"
    danger={true}
    onConfirm={() => deleteTarget && void handleDelete(deleteTarget)}
  />
</section>

<style>
  /* Fixed overlay: escapes the card's overflow clipping and paints on top.
     Position is set from the toggle button's viewport rect at open time. */
  .btn-icon {
    display: inline-flex;
    margin-right: 0.35rem;
    vertical-align: -2px;
  }
</style>
