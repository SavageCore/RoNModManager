<script lang="ts">
  import { onMount } from "svelte";
  import {
    addModIoMod,
    applyProfile,
    getConfig,
    getInstalledModGroups,
    getProfile,
    installLocalMod,
    launchGameWithGroups,
    listProfiles,
    saveProfile,
    syncModLinks,
    uninstallArchive,
    uninstallMods,
    uninstallMod,
    updateModDisplayName,
    updateModSourceUrl,
  } from "$lib/api/commands";
  import AddModModal from "$lib/components/AddModModal.svelte";
  import ConfirmModal from "$lib/components/ConfirmModal.svelte";
  import SourceIcon from "$lib/components/SourceIcon.svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import {
    ExternalLink,
    Link as LinkIcon,
    Globe,
    Pencil,
    Plus,
    Trash2,
  } from "lucide-svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import type { InstalledModGroup, Profile } from "$lib/types";
  import { modAddQueueStore } from "$lib/stores/modAddQueue";
  import { toastStore } from "$lib/stores/toast";

  let modGroups: InstalledModGroup[] = [];
  let modSearch = "";
  let modSourceFilter: "all" | "nexus" | "modio" = "all";
  let modsForActiveProfile: string[] = [];
  let showAddModModal = false;
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
  let profiles: Profile[] = [];
  let selectedProfile = "Default";
  let activeProfileName: string | null = null;
  let allInstalledGroupNames = new Set<string>();
  let pendingIncludeNewModsForActiveProfile = false;
  let hasGamePath = false;
  let isApplyingProfile = false;
  let expandedGroups: Record<string, boolean> = {};
  let isDraggingOver = false;
  let editingGroup: string | null = null;
  let editInputValue = "";
  let editingUrlGroup: string | null = null;
  let editUrlInputValue = "";

  function extractModIoInputFromDroppedText(raw: string): string | null {
    const candidates = raw
      .split(/\r?\n/)
      .map((entry) => entry.trim())
      .filter((entry) => entry.length > 0 && !entry.startsWith("#"));

    for (const candidate of candidates) {
      if (
        candidate.includes("mod.io/") ||
        /\/m\/[A-Za-z0-9_-]+/.test(candidate)
      ) {
        return candidate;
      }
    }

    return null;
  }

  function getModSource(sourceUrl?: string): "nexus" | "modio" | null {
    if (!sourceUrl) return null;
    if (sourceUrl.includes("nexusmods.com")) return "nexus";
    if (sourceUrl.includes("mod.io")) return "modio";
    return null;
  }

  // Filtering logic for modGroups
  $: filteredModGroups = modGroups.filter((group) => {
    // Filter by search
    const search = modSearch.trim().toLowerCase();
    const name = group.name.toLowerCase();
    const displayName = (group.displayName || "").toLowerCase();
    const matchesSearch =
      !search || name.includes(search) || displayName.includes(search);
    // Filter by source
    const source = getModSource(group.sourceUrl);
    const matchesSource =
      modSourceFilter === "all" || source === modSourceFilter;
    return matchesSearch && matchesSource;
  });

  async function handleModIoLinkDrop(input: string) {
    try {
      if (activeProfileName) {
        pendingIncludeNewModsForActiveProfile = true;
      }
      const result = await addModIoMod(input);
      toastStore.success(`Installed ${result.name}`);
      await refresh();
    } catch (error) {
      toastStore.error(`Failed to add mod from link: ${String(error)}`);
    }
  }

  function handleWindowDragOver(event: DragEvent) {
    const transfer = event.dataTransfer;
    if (!transfer) {
      return;
    }

    const hasFiles = transfer.types.includes("Files");
    const hasText =
      transfer.types.includes("text/uri-list") ||
      transfer.types.includes("text/plain");
    if (hasFiles || hasText) {
      isDraggingOver = true;
    }
  }

  async function handleWindowDrop(event: DragEvent) {
    const transfer = event.dataTransfer;
    isDraggingOver = false;

    if (!transfer) {
      return;
    }

    const uriList = transfer.getData("text/uri-list");
    const plainText = transfer.getData("text/plain");
    const droppedInput = extractModIoInputFromDroppedText(
      `${uriList}\n${plainText}`,
    );
    if (droppedInput) {
      await handleModIoLinkDrop(droppedInput);
    }
  }

  function handleWindowDragLeave() {
    isDraggingOver = false;
  }

  // Action to auto-focus input
  function focus(node: HTMLElement) {
    node.focus();
    return {};
  }

  const DEFAULT_PROFILE_NAME = "Default";

  async function ensureDefaultProfile(
    profileList: Profile[],
  ): Promise<Profile[]> {
    const hasDefault = profileList.some(
      (profile) => profile.name === DEFAULT_PROFILE_NAME,
    );
    if (hasDefault) {
      return profileList;
    }
    return profileList;
  }

  function sortModGroups(list: InstalledModGroup[]): InstalledModGroup[] {
    return [...list].sort((a, b) => {
      const aName = (a.displayName || a.name).toLowerCase();
      const bName = (b.displayName || b.name).toLowerCase();
      return aName.localeCompare(bName);
    });
  }

  async function refresh() {
    try {
      const [groups, config, profileList] = await Promise.all([
        getInstalledModGroups(),
        getConfig(),
        listProfiles().catch(() => []),
      ]);

      const previousGroupNames = new Set(allInstalledGroupNames);
      allInstalledGroupNames = new Set(groups.map((group) => group.name));

      activeProfileName = config.active_profile?.trim()
        ? config.active_profile
        : null;

      // Load installed mods from the active profile
      modsForActiveProfile = [];
      if (activeProfileName) {
        const activeProfile = await getProfile(activeProfileName);
        if (activeProfile) {
          modsForActiveProfile = [...activeProfile.installed_mod_names];
        }
      }

      if (activeProfileName && pendingIncludeNewModsForActiveProfile) {
        const newlyInstalled = groups
          .map((group) => group.name)
          .filter((name) => !previousGroupNames.has(name));

        if (newlyInstalled.length > 0) {
          modsForActiveProfile = Array.from(
            new Set([...modsForActiveProfile, ...newlyInstalled]),
          );
          // Persist to the active profile
          const activeProfile = await getProfile(activeProfileName);
          if (activeProfile) {
            await saveProfile(
              activeProfileName,
              activeProfile.description,
              modsForActiveProfile,
            );
          }
        }
        pendingIncludeNewModsForActiveProfile = false;
      }

      modGroups = sortModGroups(groups);
      hasGamePath = config.game_path != null;
      profiles = await ensureDefaultProfile(profileList);
      expandedGroups = Object.fromEntries(
        modGroups.map((group) => [
          group.name,
          expandedGroups[group.name] ?? false,
        ]),
      );

      // Note: With independent mod lists per profile, toggle state is no longer needed
      // Mods either belong to the active profile or they don't

      if (!profiles.some((profile) => profile.name === selectedProfile)) {
        selectedProfile = DEFAULT_PROFILE_NAME;
      }
      if (
        activeProfileName &&
        profiles.some((profile) => profile.name === activeProfileName)
      ) {
        selectedProfile = activeProfileName;
      }
    } catch (error) {
      toastStore.error(`Failed to load mods: ${String(error)}`);
    }
  }

  async function persistActiveProfileEnabledGroups(enabledGroups: string[]) {
    if (!activeProfileName) {
      return;
    }
    try {
      const activeProfile = await getProfile(activeProfileName);
      if (activeProfile) {
        await saveProfile(
          activeProfileName,
          activeProfile.description,
          enabledGroups,
        );
      }
    } catch (error) {
      console.error("Failed to persist enabled groups:", error);
      toastStore.error(`Failed to save profile state: ${String(error)}`);
    }
  }

  async function handleProfileChange() {
    if (!selectedProfile) return;
    try {
      isApplyingProfile = true;
      const profile = await applyProfile(selectedProfile);

      await refresh();

      if (hasGamePath) {
        const enabledGroups = [...profile.installed_mod_names];
        await syncModLinks(enabledGroups);
      }

      toastStore.success(`Applied profile: ${selectedProfile}`);
    } catch (error) {
      toastStore.error(`Failed to apply profile: ${String(error)}`);
    } finally {
      isApplyingProfile = false;
    }
  }

  async function launchWithSelection() {
    if (!hasGamePath) {
      toastStore.error("Game path is not configured. Open Settings first.");
      return;
    }

    try {
      if (selectedProfile) {
        const profile = await applyProfile(selectedProfile);
        await refresh();
        await launchGameWithGroups([...profile.installed_mod_names]);
        toastStore.success(`Launched game with profile: ${selectedProfile}`);
        return;
      }
    } catch (error) {
      toastStore.error(`Failed to launch game: ${String(error)}`);
    }
  }

  function toggleGroup(name: string) {
    expandedGroups[name] = !expandedGroups[name];
  }

  async function toggleGroupState(name: string) {
    if (!activeProfileName) {
      return;
    }

    try {
      // Get current profile
      const activeProfile = await getProfile(activeProfileName);
      if (!activeProfile) {
        return;
      }

      // Toggle membership in profile's installed_mod_names
      const currentMods = new Set(activeProfile.installed_mod_names);
      if (currentMods.has(name)) {
        currentMods.delete(name);
      } else {
        currentMods.add(name);
      }

      const enabledGroups = Array.from(currentMods);

      // Persist the update
      await persistActiveProfileEnabledGroups(enabledGroups);
      modsForActiveProfile = enabledGroups;

      // Sync game links if game path is configured
      if (hasGamePath) {
        await syncModLinks(enabledGroups);
      }
    } catch (error) {
      toastStore.error(`Failed to update active mods: ${String(error)}`);
    }
  }

  async function handleUninstallMod(filename: string) {
    try {
      await uninstallMod(filename);
      toastStore.success(`Uninstalled: ${filename}`);

      await refresh();
      if (hasGamePath) {
        await syncModLinks(modsForActiveProfile);
      }
    } catch (error) {
      toastStore.error(`Failed to uninstall: ${String(error)}`);
    }
  }

  async function handleUninstallArchive(group: InstalledModGroup) {
    const label = group.displayName ?? group.name;
    const isNamed = !!group.displayName;
    confirmModal = {
      isVisible: true,
      title: "Uninstall mod?",
      message: `Are you sure you want to uninstall <strong>${label}</strong>? This cannot be undone.`,
      detail: isNamed ? group.name : "",
      confirmLabel: "Uninstall",
      onConfirm: async () => {
        try {
          if (group.managedByManifest) {
            await uninstallArchive(group.name);
          } else {
            const primaryFile = group.files[0]?.name;
            if (primaryFile) {
              await uninstallMod(primaryFile);
            }
          }
          toastStore.success(`Uninstalled: ${group.name}`);
          await refresh();
          if (hasGamePath) {
            await syncModLinks(modsForActiveProfile);
          }
        } catch (error) {
          toastStore.error(
            `Failed to uninstall ${group.name}: ${String(error)}`,
          );
        }
      },
    };
  }

  async function handleUninstallAll() {
    confirmModal = {
      isVisible: true,
      title: "Uninstall all mods?",
      message:
        "This will permanently remove all installed mods. This cannot be undone.",
      confirmLabel: "Uninstall All",
      detail: "",
      onConfirm: async () => {
        try {
          await uninstallMods();
          toastStore.success("Uninstalled all mods");
          modsForActiveProfile = [];
          await refresh();
          if (hasGamePath) {
            await syncModLinks([]);
          }
        } catch (error) {
          toastStore.error(`Failed to uninstall all mods: ${String(error)}`);
        }
      },
    };
  }

  async function handleToggleAll() {
    if (!activeProfileName) {
      return;
    }

    const modSet = new Set(modsForActiveProfile);
    const allEnabled =
      modGroups.length > 0 &&
      modGroups.every((group) => modSet.has(group.name));
    const enabledGroups = allEnabled ? [] : modGroups.map((g) => g.name);

    try {
      await persistActiveProfileEnabledGroups(enabledGroups);
      modsForActiveProfile = enabledGroups;
      if (hasGamePath) {
        await syncModLinks(enabledGroups);
      }
      toastStore.success(allEnabled ? "Disabled all mods" : "Enabled all mods");
    } catch (error) {
      toastStore.error(`Failed to update mods: ${String(error)}`);
    }
  }

  function startEditingName(group: InstalledModGroup) {
    editingGroup = group.name;
    editInputValue = group.displayName || group.name;
  }

  function cancelEditingName() {
    editingGroup = null;
    editInputValue = "";
  }

  async function saveEditedName(group: InstalledModGroup) {
    const trimmed = editInputValue.trim();
    if (trimmed && trimmed !== group.name) {
      try {
        await updateModDisplayName(group.name, trimmed);
        await refresh();
        // Resort after name update to reflect new order immediately
        modGroups = sortModGroups(modGroups);
        toastStore.success("Updated mod name");
      } catch (error) {
        toastStore.error(`Failed to update name: ${String(error)}`);
      }
    }
    cancelEditingName();
  }

  function handleNameKeydown(event: KeyboardEvent, group: InstalledModGroup) {
    if (event.key === "Enter") {
      void saveEditedName(group);
    } else if (event.key === "Escape") {
      cancelEditingName();
    }
  }

  function startEditingSourceUrl(group: InstalledModGroup) {
    editingUrlGroup = group.name;
    editUrlInputValue = group.sourceUrl || "";
  }

  function cancelEditingSourceUrl() {
    editingUrlGroup = null;
    editUrlInputValue = "";
  }

  // Clean mod source URLs before saving: strip query string and fragment for Nexus/Mod.io
  function cleanModSourceUrl(url: string): string {
    try {
      const u = new URL(url);
      // Only clean for known hosts
      if (
        u.hostname.includes("nexusmods.com") ||
        u.hostname.includes("mod.io")
      ) {
        u.hash = "";
        u.search = "";
        return u.toString();
      }
      return url;
    } catch {
      return url;
    }
  }

  async function saveSourceUrl(group: InstalledModGroup) {
    let trimmed = editUrlInputValue.trim();
    if (trimmed) {
      try {
        new URL(trimmed);
      } catch {
        toastStore.error("Source URL must be a valid URL");
        return;
      }
      trimmed = cleanModSourceUrl(trimmed);
    }

    try {
      await updateModSourceUrl(group.name, trimmed);

      await refresh();
      toastStore.success("Updated source URL");
      cancelEditingSourceUrl();
    } catch (error) {
      toastStore.error(`Failed to update source URL: ${String(error)}`);
    }
  }

  function handleSourceUrlKeydown(
    event: KeyboardEvent,
    group: InstalledModGroup,
  ) {
    if (event.key === "Enter") {
      void saveSourceUrl(group);
    } else if (event.key === "Escape") {
      cancelEditingSourceUrl();
    }
  }

  async function openInstalledFile(path: string, exists: boolean) {
    if (!exists) {
      toastStore.error("File is missing on disk.");
      return;
    }

    try {
      await revealItemInDir(path);
    } catch (error) {
      toastStore.error(`Failed to open file location: ${String(error)}`);
    }
  }

  async function handleFileDrop(paths: string[]) {
    console.log("Tauri file drop received:", paths);

    if (activeProfileName) {
      pendingIncludeNewModsForActiveProfile = true;
    }

    // Reset batch counter for new queue
    modAddQueueStore.resetBatch();

    // Enqueue all files first
    const queueEntries = paths.map((filePath) => {
      const fileName = filePath.split(/[\\/]/).pop() || filePath;
      return {
        path: filePath,
        fileName,
        queueId: modAddQueueStore.enqueue(fileName),
      };
    });

    // Process each file
    for (const entry of queueEntries) {
      console.log("Installing file:", entry.path);
      modAddQueueStore.markRunning(
        entry.queueId,
        `Installing ${entry.fileName}...`,
      );

      try {
        await installLocalMod(entry.path);
        modAddQueueStore.markDone(entry.queueId, `Installed ${entry.fileName}`);
        toastStore.success(`Successfully installed: ${entry.fileName}`);
        console.log("Installation successful:", entry.fileName);
      } catch (error) {
        const message = `Failed to install ${entry.fileName}: ${String(error)}`;
        modAddQueueStore.markError(entry.queueId, message);
        toastStore.error(message);
        console.error("Installation failed:", error);
      }
    }

    await refresh();
  }

  async function handleModAdded() {
    if (activeProfileName) {
      pendingIncludeNewModsForActiveProfile = true;
    }
    await refresh();
  }

  onMount(() => {
    console.log("Mods page mounted, setting up event listeners");
    void refresh();

    const handleAppFocus = () => {
      void refresh();
    };
    const handleVisibilityChange = () => {
      if (document.visibilityState === "visible") {
        void refresh();
      }
    };
    const handleMetadataRefreshed = () => {
      void refresh();
    };

    window.addEventListener("focus", handleAppFocus);
    document.addEventListener("visibilitychange", handleVisibilityChange);
    window.addEventListener("ron:metadata-refreshed", handleMetadataRefreshed);

    const appWindow = getCurrentWindow();
    let unlistenDragDrop: (() => void) | null = null;

    // Use Tauri 2.0's onDragDropEvent API
    console.log("Setting up onDragDropEvent listener...");
    void appWindow
      .onDragDropEvent((event) => {
        console.log("onDragDropEvent fired:", event);

        if (event.payload.type === "over") {
          console.log("Drag over");
          isDraggingOver = true;
        } else if (event.payload.type === "drop") {
          console.log("File dropped!", event.payload);
          isDraggingOver = false;
          if ("paths" in event.payload && Array.isArray(event.payload.paths)) {
            console.log("Extracted paths:", event.payload.paths);
            void handleFileDrop(event.payload.paths);
          }
        } else if (event.payload.type === "leave") {
          console.log("Drag leave");
          isDraggingOver = false;
        }
      })
      .then((fn) => {
        console.log("onDragDropEvent listener registered");
        unlistenDragDrop = fn;
      });

    return () => {
      console.log("Cleaning up mods page listeners");
      window.removeEventListener("focus", handleAppFocus);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
      window.removeEventListener(
        "ron:metadata-refreshed",
        handleMetadataRefreshed,
      );
      if (unlistenDragDrop) {
        unlistenDragDrop();
      }
    };
  });
</script>

<svelte:window
  on:dragover|preventDefault={handleWindowDragOver}
  on:drop|preventDefault={handleWindowDrop}
  on:dragleave={handleWindowDragLeave}
/>

<AddModModal
  isVisible={showAddModModal}
  on:close={() => {
    showAddModModal = false;
  }}
  on:modAdded={handleModAdded}
/>

<ConfirmModal
  bind:isVisible={confirmModal.isVisible}
  title={confirmModal.title}
  message={confirmModal.message}
  detail={confirmModal.detail}
  confirmLabel={confirmModal.confirmLabel}
  onConfirm={confirmModal.onConfirm}
/>

<!-- Filter Controls -->
<div class="flex flex-col sm:flex-row gap-2 mb-4 items-center">
  <input
    class="input flex-1 min-w-0"
    type="text"
    placeholder="Search mods by name..."
    bind:value={modSearch}
    style="max-width: 320px;"
  />
  <select
    class="input"
    bind:value={modSourceFilter}
    style="width: 140px; align-self: stretch;"
    aria-label="Filter by source"
  >
    <option value="all">All Sources</option>
    <option value="modio">Mod.io</option>
    <option value="nexus">Nexus</option>
  </select>
</div>

<!-- Gale-style Mod List -->
<div
  role="region"
  aria-label="Mod list with drag and drop support"
  style="background: var(--clr-surface); border-color: {isDraggingOver
    ? 'var(--clr-primary-300)'
    : 'var(--adw-border-color)'}; border-width: {isDraggingOver
    ? '2px'
    : '1px'};"
  class="border rounded-lg p-4 transition-all {isDraggingOver
    ? 'shadow-lg'
    : ''}"
>
  <div class="flex items-center justify-between mb-4 gap-3">
    <h2 style="color: var(--clr-text);" class="text-lg font-semibold">Mods</h2>
    <div class="flex items-center gap-2">
      <button
        on:click={() => {
          showAddModModal = true;
        }}
        class="btn btn-sm btn-success"
        title="Add Mod"
      >
        <Plus size={16} class="inline mr-1" />
        Add Mod
      </button>
      <button
        class="btn btn-sm btn-danger"
        on:click={() => {
          void handleUninstallAll();
        }}
        disabled={modGroups.length === 0}
        title="Uninstall every installed mod"
      >
        <Trash2 size={16} class="inline mr-1" />
        Uninstall All
      </button>
    </div>
  </div>

  <p style="color: var(--clr-text-secondary);" class="text-sm mb-4">
    Each profile has its own set of active mods. Use the checkboxes to enable or
    disable mods for this profile.
  </p>

  {#if filteredModGroups.length > 0}
    <div class="flex items-center gap-2 mb-2">
      <label class="gale-switch" title="Toggle all mods on/off">
        <input
          type="checkbox"
          checked={modGroups.length > 0 &&
            modGroups.every((g) => modsForActiveProfile.includes(g.name))}
          on:change={() => {
            void handleToggleAll();
          }}
          disabled={modGroups.length === 0 || !activeProfileName}
        />
        <span class="gale-switch-track"></span>
      </label>
      <span style="color: var(--clr-text-secondary);" class="text-sm"
        >Toggle all</span
      >
    </div>
  {/if}

  {#if filteredModGroups.length === 0}
    <p
      style="color: var(--clr-text-secondary);"
      class="text-sm text-center py-8"
    >
      {#if modGroups.length === 0}
        No installed mods yet. Click the + button below to add mods or drag and
        drop files here.
      {:else}
        No mods match your filter.
      {/if}
    </p>
  {:else}
    <ul class="space-y-2">
      {#each filteredModGroups as group (group.name)}
        <li
          style="background: var(--clr-surface-variant); border-color: var(--adw-border-color);"
          class="rounded border"
        >
          <div class="flex items-center justify-between px-3 py-2 gap-3">
            <button
              class="flex items-center gap-2 flex-1 min-w-0 text-left cursor-pointer"
              on:click={() => toggleGroup(group.name)}
              title={expandedGroups[group.name] ? "Collapse" : "Expand"}
            >
              <span
                style="color: var(--clr-text-secondary);"
                class="text-xs w-4 text-center cursor-pointer"
              >
                {expandedGroups[group.name] ? "v" : ">"}
              </span>
              <div class="min-w-0">
                {#if editingGroup === group.name}
                  <input
                    type="text"
                    bind:value={editInputValue}
                    on:blur={() => {
                      void saveEditedName(group);
                    }}
                    on:keydown={(e) => handleNameKeydown(e, group)}
                    class="text-sm font-medium px-1 py-0.5 rounded border"
                    style="color: var(--clr-text); background: var(--clr-surface); border-color: var(--clr-primary-300);"
                    use:focus
                  />
                {:else}
                  <div class="group/name flex items-center gap-1.5">
                    <SourceIcon
                      source={getModSource(group.sourceUrl)}
                      size={16}
                    />
                    <p
                      style="color: var(--clr-text);"
                      class="text-sm font-medium truncate"
                    >
                      {group.displayName || group.name}
                    </p>
                    {#if group.sourceUrl}
                      <a
                        href={group.sourceUrl}
                        target="_blank"
                        rel="noopener noreferrer"
                        class="ml-1 flex items-center"
                        title="Open mod page in browser"
                        tabindex="-1"
                        style="color: var(--clr-primary-300);"
                      >
                        <Globe size={15} />
                      </a>
                    {/if}
                    <span
                      role="button"
                      tabindex="0"
                      on:click|stopPropagation={() => startEditingName(group)}
                      on:keydown={(e) => {
                        if (e.key === "Enter" || e.key === " ") {
                          e.preventDefault();
                          e.stopPropagation();
                          startEditingName(group);
                        }
                      }}
                      class="opacity-0 group-hover/name:opacity-100 transition-opacity cursor-pointer"
                      title="Edit display name"
                    >
                      <Pencil
                        size={14}
                        style="color: var(--clr-text-secondary);"
                      />
                    </span>
                  </div>
                {/if}
                <p style="color: var(--clr-text-secondary);" class="text-xs">
                  {group.files.length} installed file{group.files.length === 1
                    ? ""
                    : "s"}
                </p>
              </div>
            </button>

            <div class="flex items-center gap-2">
              <label
                class="gale-switch"
                title={`${modsForActiveProfile.includes(group.name) ? "Disable" : "Enable"} ${group.displayName ?? group.name}`}
              >
                <input
                  type="checkbox"
                  checked={modsForActiveProfile.includes(group.name)}
                  on:change={() => toggleGroupState(group.name)}
                  disabled={!activeProfileName}
                />
                <span class="gale-switch-track"></span>
              </label>

              <button
                class="icon-btn-danger"
                title={`Uninstall ${group.displayName ?? group.name}`}
                on:click={() => {
                  void handleUninstallArchive(group);
                }}
              >
                <Trash2 size={14} aria-hidden="true" />
              </button>
            </div>
          </div>

          {#if expandedGroups[group.name]}
            <div
              style="border-color: var(--adw-border-color);"
              class="border-t px-3 py-2 space-y-1"
            >
              <div class="flex items-center gap-2 text-xs mb-2">
                <span
                  style="color: var(--clr-text-secondary);"
                  class="inline-flex items-center gap-1 flex-shrink-0"
                >
                  <LinkIcon size={13} />
                  Source
                </span>
                {#if editingUrlGroup === group.name}
                  <input
                    type="url"
                    bind:value={editUrlInputValue}
                    on:keydown={(e) => handleSourceUrlKeydown(e, group)}
                    class="flex-1 min-w-0 text-xs px-2 py-1 rounded border"
                    style="color: var(--clr-text); background: var(--clr-surface); border-color: var(--clr-primary-300);"
                    placeholder="https://www.nexusmods.com/..."
                    use:focus
                  />
                  <button
                    class="btn primary btn-sm"
                    on:click={() => {
                      void saveSourceUrl(group);
                    }}
                  >
                    Save
                  </button>
                  <button class="btn btn-sm" on:click={cancelEditingSourceUrl}>
                    Cancel
                  </button>
                {:else}
                  <div class="flex items-center gap-2 flex-1 min-w-0">
                    {#if group.sourceUrl}
                      <a
                        href={group.sourceUrl}
                        target="_blank"
                        rel="noreferrer"
                        style="color: var(--clr-primary-300);"
                        class="truncate inline-flex items-center gap-1 flex-1 min-w-0"
                        title={group.sourceUrl}
                      >
                        <span class="truncate">{group.sourceUrl}</span>
                        <ExternalLink size={12} class="flex-shrink-0" />
                      </a>
                    {:else}
                      <span
                        style="color: var(--clr-text-secondary);"
                        class="truncate">No source URL saved</span
                      >
                    {/if}
                  </div>
                  <button
                    class="btn btn-sm flex-shrink-0"
                    on:click={() => startEditingSourceUrl(group)}
                    title="Edit source URL"
                  >
                    Edit Link
                  </button>
                {/if}
              </div>

              {#each group.files as file (file.path)}
                <div class="flex items-center justify-between gap-3 text-xs">
                  <button
                    style="color: var(--clr-text);"
                    class="truncate text-left hover:underline"
                    title={file.path}
                    on:click={() => {
                      void openInstalledFile(file.path, file.exists);
                    }}
                  >
                    {file.name}
                  </button>
                  <span
                    style="color: {file.exists
                      ? 'var(--clr-success-300)'
                      : 'var(--clr-danger-300)'};"
                    class="flex-shrink-0"
                    title={file.path}
                  >
                    {file.exists ? "installed" : "missing"}
                  </span>
                </div>
              {/each}
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
</style>
