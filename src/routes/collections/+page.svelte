<script lang="ts">
  import {
    createCollection,
    deleteCollection,
    getCollectionColors,
    getCollectionMods,
    getCollections,
    getConfig,
    getInstalledModGroups,
    getProfile,
    removeModFromCollection,
    renameCollection,
    setCollectionColor,
    toggleCollection,
  } from "$lib/api/commands";
  import ConfirmModal from "$lib/components/ConfirmModal.svelte";
  import EditCollectionModal from "$lib/components/EditCollectionModal.svelte";
  import {
    incognitoMode,
    DUMMY_COLLECTIONS,
    DUMMY_COLLECTION_COLORS,
    DUMMY_MOD_GROUPS,
  } from "$lib/stores/incognitoMode";
  import { toastStore } from "$lib/stores/toast";
  import { X } from "lucide-svelte";
  import { onMount } from "svelte";

  let rawCollectionMods: Record<string, string[]> = {};
  let rawCollections: Record<string, boolean> = {};
  let rawCollectionColors: Record<string, string> = {};
  let rawModDisplayNames: Record<string, string> = {};
  $: collectionMods = $incognitoMode ? DUMMY_COLLECTIONS : rawCollectionMods;
  $: collections = $incognitoMode
    ? Object.fromEntries(Object.keys(DUMMY_COLLECTIONS).map((k) => [k, true]))
    : rawCollections;
  $: collectionColors = $incognitoMode
    ? DUMMY_COLLECTION_COLORS
    : rawCollectionColors;
  $: modDisplayNames = $incognitoMode
    ? Object.fromEntries(
        DUMMY_MOD_GROUPS.map((g) => [g.name, g.displayName?.trim() || g.name]),
      )
    : rawModDisplayNames;

  let activeProfileName: string | null = null;
  let activeProfileEnabledCount = 0;
  let newCollectionName = "";
  let loading = false;
  let hasLoadedOnce = false;
  let editModal: { isVisible: boolean; name: string; color: string | null } = {
    isVisible: false,
    name: "",
    color: null,
  };
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
  function escapeHtml(v: string): string {
    return v
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;")
      .replaceAll("'", "&#39;");
  }
  function renderDeleteModList(mods: string[]): string {
    return sortedMods(mods)
      .map((m) => escapeHtml(resolveModName(m)))
      .join("<br>");
  }

  async function refresh() {
    loading = true;
    try {
      const [
        collectionState,
        profileCollectionMods,
        config,
        installedGroups,
        colors,
      ] = await Promise.all([
        getCollections(),
        getCollectionMods(),
        getConfig(),
        getInstalledModGroups().catch(() => []),
        getCollectionColors(),
      ]);
      rawCollectionMods = profileCollectionMods;
      rawCollectionColors = colors;
      activeProfileName = config.active_profile;
      rawModDisplayNames = Object.fromEntries(
        installedGroups.map((g) => [g.name, g.displayName?.trim() || g.name]),
      );
      if (activeProfileName) {
        const profile = await getProfile(activeProfileName);
        activeProfileEnabledCount = profile?.installed_mod_names.length ?? 0;
        const enabledSet = new Set(profile?.installed_mod_names ?? []);
        rawCollections = Object.fromEntries(
          Object.entries(collectionState).map(([name, enabled]) => {
            const mods = profileCollectionMods[name] ?? [];
            return [
              name,
              enabled ||
                (mods.length > 0 && mods.every((m) => enabledSet.has(m))),
            ];
          }),
        );
      } else {
        activeProfileEnabledCount = 0;
        rawCollections = collectionState;
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
      message: `Are you sure you want to delete <strong>${escapeHtml(name)}</strong>? This will remove the collection grouping${modCount > 0 ? ` for ${modCount} mod${modCount === 1 ? "" : "s"}` : ""}.${modCount > 0 ? `<div style="margin-top:0.75rem;color:var(--clr-text-secondary);">${modListMarkup}</div>` : ""}`,
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
  function openEditModal(name: string) {
    editModal = {
      isVisible: true,
      name,
      color: collectionColors[name] ?? null,
    };
  }
  async function onSaveEdit(newName: string, newColor: string | null) {
    const oldName = editModal.name;
    const oldColor = collectionColors[oldName] ?? null;
    try {
      if (newName !== oldName) await renameCollection(oldName, newName);
      const effectiveName = newName !== oldName ? newName : oldName;
      if (newColor !== oldColor)
        await setCollectionColor(effectiveName, newColor);
      await refresh();
      toastStore.success(`Updated collection.`);
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
    return () =>
      window.removeEventListener(
        "ron:collections-changed",
        onCollectionsChanged,
      );
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
<EditCollectionModal
  bind:isVisible={editModal.isVisible}
  initialName={editModal.name}
  initialColor={editModal.color}
  onSave={onSaveEdit}
/>

<section class="prefs-page">
  <h1 style="color: var(--clr-text);" class="text-2xl font-bold">
    Collections
  </h1>
  <p style="color: var(--clr-text-secondary);" class="text-sm mt-1">
    Group installed mods and toggle whole collections for the active profile.
    Right-click a mod in Mods to add it to a collection.
  </p>
  {#if !activeProfileName && hasLoadedOnce}<p
      style="color: var(--clr-text-secondary);"
      class="text-xs mt-1"
    >
      No active profile — create/select a profile first.
    </p>{/if}

  <div class="prefs-group">
    <div class="prefs-group-title">Collections</div>
    <div class="prefs-group-desc">
      {Object.keys(collectionMods).length} collection{Object.keys(
        collectionMods,
      ).length === 1
        ? ""
        : "s"}
    </div>
    <div class="prefs-boxed-list">
      <div class="prefs-row">
        <div class="prefs-row-text">
          <div class="prefs-row-title">New collection</div>
          <div class="prefs-row-subtitle">Create an empty collection</div>
        </div>
        <div class="prefs-row-suffix" style="flex:1; max-width: 320px;">
          <input
            class="input"
            type="text"
            bind:value={newCollectionName}
            placeholder="Name"
            disabled={!activeProfileName}
            on:keydown={(e) => {
              if (e.key === "Enter") void onCreateCollection();
            }}
          />
          <button
            class="btn primary btn-sm"
            on:click={onCreateCollection}
            disabled={!activeProfileName}>Create</button
          >
        </div>
      </div>

      {#if loading && !hasLoadedOnce}
        <div class="prefs-row">
          <span class="prefs-row-subtitle">Loading collections…</span>
        </div>
      {:else if Object.keys(collectionMods).length === 0}
        <div class="prefs-row">
          <span class="prefs-row-subtitle">No collections yet.</span>
        </div>
      {:else}
        {#each sortedCollectionEntries as [name, mods] (name)}
          {@const color = collectionColors[name] ?? null}
          <div class="prefs-row prefs-row--collection">
            <div class="prefs-row-main">
              <div class="prefs-row-text">
                {#if color}<span
                    class="collection-pill font-medium"
                    style="background: color-mix(in srgb, {color} 15%, transparent); border-color: {color}; color: {color};"
                    >{name}</span
                  >{:else}<div class="prefs-row-title">{name}</div>{/if}
                <div class="prefs-row-subtitle">
                  {mods.length} mod{mods.length === 1 ? "" : "s"}
                </div>
              </div>
              <div class="prefs-row-suffix">
                <label
                  class="gale-switch"
                  title={`${(collections[name] ?? false) ? "Disable" : "Enable"} ${name}`}
                  aria-label={`${(collections[name] ?? false) ? "Disable" : "Enable"} ${name}`}
                >
                  <input
                    type="checkbox"
                    checked={collections[name] ?? false}
                    on:change={(e) => onToggle(name, e.currentTarget.checked)}
                    disabled={!activeProfileName}
                  />
                  <span class="gale-switch-track"></span>
                </label>
                <button
                  class="btn btn-sm"
                  on:click={() => openEditModal(name)}
                  disabled={!activeProfileName}>Edit</button
                >
                <button
                  class="btn btn-sm danger"
                  on:click={() => void onDeleteCollection(name)}
                  disabled={!activeProfileName}>Delete</button
                >
              </div>
            </div>
            {#if mods.length > 0}
              <div class="collection-chips">
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
                      on:click={() => void onRemoveMod(name, modName)}
                      title={`Remove ${resolveModName(modName)} from ${name}`}
                      aria-label={`Remove ${resolveModName(modName)} from ${name}`}
                      ><X size={12} aria-hidden="true" /></button
                    >
                  </span>
                {/each}
              </div>
            {/if}
          </div>
        {/each}
      {/if}
    </div>
  </div>
</section>

<style>
  .prefs-row--collection {
    flex-direction: column;
    align-items: stretch;
    gap: 0.5rem;
  }
  .prefs-row-main {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    width: 100%;
  }
  .collection-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
  }
  .collection-pill {
    display: inline-block;
    padding: 0.125rem 0.5rem;
    border-radius: 9999px;
    border: 1px solid;
    font-size: 0.875rem;
    line-height: 1.5;
  }
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
