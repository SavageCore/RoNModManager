<script lang="ts">
  import { onMount } from "svelte";
  import { X } from "lucide-svelte";
  import ConfirmModal from "$lib/components/ConfirmModal.svelte";
  import {
    createCollection,
    deleteCollection,
    getCollections,
    getConfig,
    getCollectionMods,
    getInstalledModGroups,
    getProfile,
    removeModFromCollection,
    toggleCollection,
  } from "$lib/api/commands";
  import { toastStore } from "$lib/stores/toast";

  let collectionMods: Record<string, string[]> = {};
  let collections: Record<string, boolean> = {};
  let activeProfileName: string | null = null;
  let activeProfileEnabledCount = 0;
  let newCollectionName = "";
  let modDisplayNames: Record<string, string> = {};
  let loading = false;
  let hasLoadedOnce = false;
  let confirmModal: {
    isVisible: boolean;
    title: string;
    message: string;
    detail: string;
    confirmLabel: string;
    onConfirm: () => void;
  } = {
    isVisible: false,
    title: "",
    message: "",
    detail: "",
    confirmLabel: "Confirm",
    onConfirm: () => {},
  };

  $: sortedCollectionEntries = Object.entries(collectionMods).sort((a, b) =>
    a[0].localeCompare(b[0], undefined, { sensitivity: "base" }),
  );

  function sortedMods(mods: string[]): string[] {
    return [...mods].sort((a, b) =>
      resolveModName(a).localeCompare(resolveModName(b), undefined, {
        sensitivity: "base",
      }),
    );
  }

  function escapeHtml(value: string): string {
    return value
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;")
      .replaceAll("'", "&#39;");
  }

  function renderDeleteModList(mods: string[]): string {
    return sortedMods(mods)
      .map((modName) => escapeHtml(resolveModName(modName)))
      .join("<br>");
  }

  async function refresh() {
    loading = true;
    try {
      const [collectionState, profileCollectionMods, config, installedGroups] =
        await Promise.all([
          getCollections(),
          getCollectionMods(),
          getConfig(),
          getInstalledModGroups().catch(() => []),
        ]);

      collections = collectionState;
      collectionMods = profileCollectionMods;
      activeProfileName = config.active_profile;
      modDisplayNames = Object.fromEntries(
        installedGroups.map((group) => [
          group.name,
          group.displayName?.trim() || group.name,
        ]),
      );

      if (activeProfileName) {
        const profile = await getProfile(activeProfileName);
        activeProfileEnabledCount = profile?.installed_mod_names.length ?? 0;
      } else {
        activeProfileEnabledCount = 0;
      }
    } catch (error) {
      toastStore.error(`Failed to load collections: ${String(error)}`);
    } finally {
      loading = false;
      hasLoadedOnce = true;
    }
  }

  function resolveModName(modName: string): string {
    return modDisplayNames[modName] ?? modName;
  }

  async function onCreateCollection() {
    const name = newCollectionName.trim();
    if (!name) {
      toastStore.error("Collection name is required.");
      return;
    }
    try {
      await createCollection(name, []);
      newCollectionName = "";
      await refresh();
      toastStore.success(`Created collection ${name}.`);
    } catch (error) {
      toastStore.error(`Failed to create collection: ${String(error)}`);
    }
  }

  async function onDeleteCollection(name: string) {
    const mods = collectionMods[name] ?? [];
    const modCount = mods.length;
    const modListMarkup = modCount > 0 ? renderDeleteModList(mods) : "";
    confirmModal = {
      isVisible: true,
      title: "Delete collection?",
      message: `Are you sure you want to delete <strong>${escapeHtml(name)}</strong>? This will remove the collection grouping${modCount > 0 ? ` for ${modCount} mod${modCount === 1 ? "" : "s"}` : ""}.${modCount > 0 ? `<div style="margin-top: 0.75rem; color: var(--clr-text-secondary);">${modListMarkup}</div>` : ""}`,
      detail: "",
      confirmLabel: "Delete",
      onConfirm: async () => {
        try {
          await deleteCollection(name);
          await refresh();
          toastStore.success(`Deleted collection ${name}.`);
        } catch (error) {
          toastStore.error(`Failed to delete collection: ${String(error)}`);
        }
      },
    };
  }

  async function onRemoveMod(collectionName: string, modName: string) {
    try {
      await removeModFromCollection(collectionName, modName);
      await refresh();
      toastStore.success(`Removed ${modName} from ${collectionName}.`);
    } catch (error) {
      toastStore.error(`Failed to remove mod: ${String(error)}`);
    }
  }

  async function onToggle(name: string, enabled: boolean) {
    try {
      await toggleCollection(name, enabled);
      collections[name] = enabled;
      await refresh();
      toastStore.success(
        `${name} ${enabled ? "enabled" : "disabled"} for profile ${activeProfileName ?? "(none)"}.`,
      );
    } catch (error) {
      toastStore.error(`Failed to update collection: ${String(error)}`);
    }
  }

  onMount(() => {
    void refresh();

    const onCollectionsChanged = () => {
      void refresh();
    };

    window.addEventListener("ron:collections-changed", onCollectionsChanged);

    return () => {
      window.removeEventListener(
        "ron:collections-changed",
        onCollectionsChanged,
      );
    };
  });
</script>

<ConfirmModal
  bind:isVisible={confirmModal.isVisible}
  title={confirmModal.title}
  message={confirmModal.message}
  detail={confirmModal.detail}
  confirmLabel={confirmModal.confirmLabel}
  onConfirm={confirmModal.onConfirm}
/>

<section class="card">
  <h1 style="color: var(--clr-text);" class="mb-4 text-2xl font-semibold">
    Collections
  </h1>
  <p style="color: var(--clr-text-secondary);" class="text-sm">
    Group installed mods and toggle whole collections for the active profile.
  </p>

  <div class="mt-4 flex items-center gap-2">
    <input
      class="input"
      type="text"
      bind:value={newCollectionName}
      placeholder="New collection name"
      disabled={!activeProfileName}
      on:keydown={(event) => {
        if (event.key === "Enter") {
          void onCreateCollection();
        }
      }}
    />
    <button
      class="btn primary"
      on:click={onCreateCollection}
      disabled={!activeProfileName}
    >
      Create
    </button>
  </div>

  {#if loading && !hasLoadedOnce}
    <p style="color: var(--clr-text-secondary);" class="mt-4 text-sm">
      Loading collections...
    </p>
  {:else if Object.keys(collectionMods).length === 0}
    <p style="color: var(--clr-text-secondary);" class="mt-4 text-sm">
      No collections yet. Create one here or right-click a mod in Mods and add
      it to a collection.
    </p>
  {:else}
    <ul class="mt-4 space-y-2">
      {#each sortedCollectionEntries as [name, mods] (name)}
        <li
          style="background: var(--clr-surface); border-color: var(--adw-border-color); color: var(--clr-text);"
          class="rounded-lg border px-3 py-2 text-sm"
        >
          <div class="flex items-start justify-between gap-3">
            <div class="min-w-0">
              <p class="font-medium">{name}</p>
              <p style="color: var(--clr-text-secondary);" class="text-xs">
                {mods.length} mod{mods.length === 1 ? "" : "s"}
              </p>
              {#if mods.length > 0}
                <div class="mt-2 flex flex-wrap gap-1">
                  {#each sortedMods(mods) as modName (modName)}
                    <span
                      style="background: var(--clr-surface-variant); border-color: var(--adw-border-color);"
                      class="inline-flex items-center gap-1 rounded border px-2 py-0.5 text-xs"
                    >
                      <span class="truncate max-w-[16rem]" title={modName}
                        >{resolveModName(modName)}</span
                      >
                      <button
                        class="chip-remove-btn"
                        on:click={() => {
                          void onRemoveMod(name, modName);
                        }}
                        title={`Remove ${resolveModName(modName)} from ${name}`}
                        aria-label={`Remove ${resolveModName(modName)} from ${name}`}
                      >
                        <X size={12} aria-hidden="true" />
                      </button>
                    </span>
                  {/each}
                </div>
              {/if}
            </div>
            <div class="flex items-center gap-2">
              <label
                class="gale-switch"
                title={`${(collections[name] ?? false) ? "Disable" : "Enable"} collection ${name}`}
                aria-label={`${(collections[name] ?? false) ? "Disable" : "Enable"} collection ${name}`}
              >
                <input
                  type="checkbox"
                  checked={collections[name] ?? false}
                  aria-label={`${(collections[name] ?? false) ? "Disable" : "Enable"} collection ${name}`}
                  on:change={(event) =>
                    onToggle(
                      name,
                      (event.currentTarget as HTMLInputElement).checked,
                    )}
                  disabled={!activeProfileName}
                />
                <span class="gale-switch-track"></span>
              </label>
              <button
                class="btn btn-sm danger"
                on:click={() => {
                  void onDeleteCollection(name);
                }}
                disabled={!activeProfileName}
              >
                Delete
              </button>
            </div>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .chip-remove-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 1rem;
    height: 1rem;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--clr-text-secondary);
    cursor: pointer;
    transition:
      background-color 120ms ease,
      color 120ms ease;
  }

  .chip-remove-btn:hover {
    background: color-mix(in srgb, var(--clr-danger-300) 18%, transparent);
    color: var(--clr-danger-300);
  }

  .chip-remove-btn:focus-visible {
    outline: 2px solid var(--clr-primary-300);
    outline-offset: 1px;
  }
</style>
