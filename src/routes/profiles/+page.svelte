<script lang="ts">
  import { onMount } from "svelte";
  import type { Profile } from "$lib/types";
  import * as commands from "$lib/api/commands";

  let profiles: Profile[] = [];
  let loading = false;
  let error: string | null = null;
  let showForm = false;
  let formName = "";
  let formDescription = "";
  let formCollections: string[] = [];
  let availableCollections: string[] = [];
  let editingProfile: Profile | null = null;

  onMount(async () => {
    await loadProfiles();
    await loadAvailableCollections();
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

  async function loadAvailableCollections() {
    try {
      const collections = await commands.getModpackCollections();
      availableCollections = Object.keys(collections);
    } catch (err) {
      console.warn("Failed to load collections:", err);
    }
  }

  function openForm(profile?: Profile) {
    if (profile) {
      editingProfile = profile;
      formName = profile.name;
      formDescription = profile.description || "";
      formCollections = [...profile.enabled_collections];
    } else {
      editingProfile = null;
      formName = "";
      formDescription = "";
      formCollections = [];
    }
    showForm = true;
  }

  function closeForm() {
    showForm = false;
    editingProfile = null;
    formName = "";
    formDescription = "";
    formCollections = [];
  }

  async function handleSubmit() {
    try {
      error = null;
      if (!formName.trim()) {
        error = "Profile name is required";
        return;
      }

      await commands.saveProfile(
        formName,
        formDescription || null,
        formCollections,
      );

      await loadProfiles();
      closeForm();
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
      } catch (err) {
        error = err instanceof Error ? err.message : String(err);
      }
    }
  }

  async function handleApply(name: string) {
    try {
      error = null;
      const profile = await commands.applyProfile(name);
      // Update local state or show confirmation
      console.log("Applied profile:", profile);
      // Optionally reload profiles to show updated state
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }

  function toggleCollection(name: string) {
    if (formCollections.includes(name)) {
      formCollections = formCollections.filter((c) => c !== name);
    } else {
      formCollections = [...formCollections, name];
    }
  }
</script>

<div class="container mx-auto px-4 py-8 max-w-2xl">
  <div class="mb-6">
    <h1 class="text-3xl font-bold mb-2">Mod Profiles</h1>
    <p class="text-gray-600">
      Save and load different mod collection configurations
    </p>
  </div>

  {#if error}
    <div
      class="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded mb-4"
    >
      {error}
    </div>
  {/if}

  {#if !showForm}
    <button
      on:click={() => openForm()}
      class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded mb-6"
    >
      + Create New Profile
    </button>
  {/if}

  {#if showForm}
    <div class="bg-gray-100 p-6 rounded-lg mb-6">
      <h2 class="text-2xl font-bold mb-4">
        {editingProfile ? "Edit Profile" : "Create New Profile"}
      </h2>

      <div class="mb-4">
        <label class="block text-gray-700 font-bold mb-2">Name</label>
        <input
          type="text"
          bind:value={formName}
          class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:border-blue-500"
          placeholder="Profile name"
        />
      </div>

      <div class="mb-4">
        <label class="block text-gray-700 font-bold mb-2">
          Description (optional)
        </label>
        <textarea
          bind:value={formDescription}
          class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:border-blue-500"
          placeholder="Profile description"
          rows="3"
        />
      </div>

      <div class="mb-4">
        <label class="block text-gray-700 font-bold mb-2">Collections</label>
        <div class="space-y-2">
          {#each availableCollections as collection (collection)}
            <label class="flex items-center">
              <input
                type="checkbox"
                checked={formCollections.includes(collection)}
                on:change={() => toggleCollection(collection)}
                class="mr-2"
              />
              <span>{collection}</span>
            </label>
          {/each}
        </div>
      </div>

      <div class="flex gap-2">
        <button
          on:click={handleSubmit}
          class="bg-green-500 hover:bg-green-700 text-white font-bold py-2 px-4 rounded"
        >
          Save Profile
        </button>
        <button
          on:click={closeForm}
          class="bg-gray-500 hover:bg-gray-700 text-white font-bold py-2 px-4 rounded"
        >
          Cancel
        </button>
      </div>
    </div>
  {/if}

  <div class="space-y-3">
    {#if loading}
      <p class="text-gray-600">Loading profiles...</p>
    {:else if profiles.length === 0}
      <p class="text-gray-600">
        No profiles created yet. Create one to get started!
      </p>
    {:else}
      {#each profiles as profile (profile.name)}
        <div
          class="bg-white border border-gray-300 rounded-lg p-4 hover:shadow-lg transition-shadow"
        >
          <div class="flex justify-between items-start mb-2">
            <div>
              <h3 class="text-lg font-bold">{profile.name}</h3>
              {#if profile.description}
                <p class="text-gray-600 text-sm">{profile.description}</p>
              {/if}
            </div>
            <span class="text-xs text-gray-500">
              {new Date(profile.created_at).toLocaleDateString()}
            </span>
          </div>

          <div class="mb-3">
            <p class="text-sm text-gray-600">
              Collections: {profile.enabled_collections.length > 0
                ? profile.enabled_collections.join(", ")
                : "None"}
            </p>
          </div>

          <div class="flex gap-2">
            <button
              on:click={() => handleApply(profile.name)}
              class="bg-green-500 hover:bg-green-700 text-white font-bold py-1 px-3 rounded text-sm"
            >
              Apply
            </button>
            <button
              on:click={() => openForm(profile)}
              class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-1 px-3 rounded text-sm"
            >
              Edit
            </button>
            <button
              on:click={() => handleDelete(profile.name)}
              class="bg-red-500 hover:bg-red-700 text-white font-bold py-1 px-3 rounded text-sm"
            >
              Delete
            </button>
          </div>
        </div>
      {/each}
    {/if}
  </div>
</div>

<style>
  :global(body) {
    @apply bg-gray-50;
  }
</style>
