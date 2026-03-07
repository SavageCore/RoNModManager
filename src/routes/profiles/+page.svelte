<script lang="ts">
  import { onMount } from "svelte";
  import type { Profile } from "$lib/types";
  import * as commands from "$lib/api/commands";
  import { toastStore } from "$lib/stores/toast";

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
      if (!formName.trim()) {
        error = "Profile name is required";
        return;
      }

      const isNewProfile = !editingProfile;
      await commands.saveProfile(
        formName,
        formDescription || null,
        formEnabledGroups,
      );

      if (isNewProfile) {
        const profile = await commands.applyProfile(formName);
        const config = await commands.getConfig();
        if (config.game_path) {
          await commands.syncModLinks(profile.installed_mod_names);
        }

        window.dispatchEvent(
          new CustomEvent("ron:profile-changed", {
            detail: { name: formName },
          }),
        );
      }

      await loadProfiles();
      closeForm();
      toastStore.success(
        isNewProfile
          ? `Profile created and switched: ${formName}`
          : "Profile updated successfully.",
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
      if (config.game_path) {
        await commands.syncModLinks(profile.installed_mod_names);
      }

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
</script>

<div class="mx-auto max-w-3xl px-4 py-8">
  <div class="mb-6">
    <h1 style="color: var(--clr-text);" class="text-3xl font-bold mb-2">
      Mod Profiles
    </h1>
    <p style="color: var(--clr-text-secondary);">
      Save and load different installed-mod configurations.
    </p>
    <p style="color: var(--clr-text-secondary);" class="text-sm mt-1">
      Profiles share one mod archive/file store, so switching profiles does not
      duplicate data.
    </p>
  </div>

  {#if error}
    <div class="message-box mb-4 rounded-lg px-4 py-3">
      {error}
    </div>
  {/if}

  {#if !showForm}
    <button on:click={() => openForm()} class="btn primary mb-6">
      + Create New Profile
    </button>
  {/if}

  {#if showForm}
    <div
      style="background: var(--clr-surface); border-color: var(--adw-border-color);"
      class="card mb-6 rounded-lg border p-6"
    >
      <h2 style="color: var(--clr-text);" class="text-2xl font-bold mb-4">
        {editingProfile ? "Edit Profile" : "Create New Profile"}
      </h2>

      <div class="mb-4">
        <label
          for="profile-name"
          style="color: var(--clr-text);"
          class="mb-2 block font-semibold">Name</label
        >
        <input
          id="profile-name"
          type="text"
          bind:value={formName}
          class="input"
          placeholder="Profile name"
        />
      </div>

      <div class="mb-4">
        <label
          for="profile-description"
          style="color: var(--clr-text);"
          class="mb-2 block font-semibold"
        >
          Description (optional)
        </label>
        <textarea
          id="profile-description"
          bind:value={formDescription}
          class="textarea"
          placeholder="Profile description"
          rows="3"
        ></textarea>
      </div>

      <div class="flex gap-2">
        <button on:click={handleSubmit} class="btn primary">
          Save Profile
        </button>
        <button on:click={closeForm} class="btn"> Cancel </button>
      </div>
    </div>
  {/if}

  <div class="space-y-3">
    {#if loading}
      <p style="color: var(--clr-text-secondary);">Loading profiles...</p>
    {:else if profiles.length === 0}
      <p style="color: var(--clr-text-secondary);">
        No profiles created yet. Create one to get started!
      </p>
    {:else}
      {#each profiles as profile (profile.name)}
        <div
          style="background: var(--clr-surface); border-color: var(--adw-border-color);color:var(--clr-text);"
          class="card rounded-lg border p-4"
        >
          <div class="flex justify-between items-start mb-2">
            <div>
              <h3 style="color: var(--clr-text);" class="text-lg font-bold">
                {profile.name}
              </h3>
              {#if profile.description}
                <p style="color: var(--clr-text-secondary);" class="text-sm">
                  {profile.description}
                </p>
              {/if}
            </div>
            <span style="color: var(--clr-text-secondary);" class="text-xs">
              {new Date(profile.created_at).toLocaleDateString()}
            </span>
          </div>

          <div class="mb-3">
            <p style="color: var(--clr-text-secondary);" class="text-sm">
              Enabled mods: {profile.installed_mod_names.length > 0
                ? profile.installed_mod_names.join(", ")
                : "None"}
            </p>
          </div>

          <div class="flex gap-2">
            <button
              on:click={() => handleApply(profile.name)}
              class="btn btn-sm primary"
            >
              Apply
            </button>
            <button on:click={() => openForm(profile)} class="btn btn-sm">
              Edit
            </button>
            <button
              on:click={() => handleDelete(profile.name)}
              class="btn btn-sm danger"
            >
              Delete
            </button>
          </div>
        </div>
      {/each}
    {/if}
  </div>
</div>
