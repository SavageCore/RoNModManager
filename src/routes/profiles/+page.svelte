<script lang="ts">
  import * as commands from "$lib/api/commands";
  import { toastStore } from "$lib/stores/toast";
  import type { Profile } from "$lib/types";
  import { get } from "svelte/store";
  import { onMount } from "svelte";
  import { incognitoMode, DUMMY_PROFILES } from "$lib/stores/incognitoMode";

  $: effectiveProfiles = $incognitoMode ? DUMMY_PROFILES : profiles;

  let profiles: Profile[] = [];
  let loading = false;
  let error: string | null = null;
  let showForm = false;
  let formName = "";
  let formDescription = "";
  let formEnabledGroups: string[] = [];
  let editingProfile: Profile | null = null;

  onMount(async () => {
    await loadProfiles();
  });

  async function loadProfiles() {
    try {
      loading = true;
      error = null;
      if (get(incognitoMode)) return;
      profiles = await commands.listProfiles();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
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
      error = null;
      const name = formName.trim();
      if (!name) {
        error = "Profile name is required";
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
        if (config.game_path)
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
      error = err instanceof Error ? err.message : String(err);
    }
  }

  async function handleDelete(name: string) {
    if (confirm(`Are you sure you want to delete profile "${name}"?`)) {
      try {
        error = null;
        await commands.deleteProfile(name);
        await loadProfiles();
        toastStore.success(`Profile "${name}" deleted successfully.`);
      } catch (err) {
        error = err instanceof Error ? err.message : String(err);
      }
    }
  }

  async function handleApply(name: string) {
    try {
      error = null;
      const profile = await commands.applyProfile(name);
      const config = await commands.getConfig();
      if (config.game_path)
        await commands.syncModLinks(profile.installed_mod_names);
      window.dispatchEvent(
        new CustomEvent("ron:profile-changed", { detail: { name } }),
      );
      toastStore.success(
        `Applied profile: ${name} (${profile.installed_mod_names.length} mod group${profile.installed_mod_names.length === 1 ? "" : "s"} enabled)`,
      );
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
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
      error = "Profile name is required";
      return;
    }
    try {
      error = null;
      await commands.duplicateProfile(name, trimmed);
      await loadProfiles();
      toastStore.success(`Profile duplicated: ${trimmed}`);
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
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

  {#if error}
    <div class="message-box mt-4 rounded-lg px-4 py-3">{error}</div>
  {/if}

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
      {#if loading && !$incognitoMode}
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
                class="btn btn-sm primary">Apply</button
              >
              <button
                on:click={() => handleDuplicate(profile.name)}
                class="btn btn-sm">Duplicate</button
              >
              <button on:click={() => openForm(profile)} class="btn btn-sm"
                >Edit</button
              >
              <button
                on:click={() => handleDelete(profile.name)}
                class="btn btn-sm danger">Delete</button
              >
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
</section>
