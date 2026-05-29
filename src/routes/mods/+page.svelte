<script lang="ts">
  import {
    addModIoMod,
    addModToCollection,
    removeModFromCollection,
    applyProfile,
    createCollection,
    getConfig,
    getArchivePakFiles,
    getInstalledModGroups,
    getProfile,
    installLocalMod,
    launchGameWithGroups,
    listProfiles,
    saveProfile,
    setModTags,
    deleteTag,
    syncModLinks,
    uninstallArchive,
    uninstallMod,
    uninstallMods,
    updateModDisplayName,
    updateModSourceUrl,
    getAddonMap,
    setAddonMap,
    getBrokenMods,
    setModBroken,
    clearModBroken,
    getNoWorldGenMods,
    setModNoWorldGen,
    clearModNoWorldGen,
  } from "$lib/api/commands";
  import AddModModal from "$lib/components/AddModModal.svelte";
  import { addModpackPanelStore } from "$lib/stores/addModpackPanelStore";
  import PakFileSelectionModal from "$lib/components/PakFileSelectionModal.svelte";
  import {
    pakSelectionStore,
    requestPakSelection,
  } from "$lib/stores/pakSelection";
  import NexusFileSelectionModal from "$lib/components/NexusFileSelectionModal.svelte";
  import { nexusFileSelectionStore } from "$lib/stores/nexusFileSelection";
  import NexusFreeDownloadModal from "$lib/components/NexusFreeDownloadModal.svelte";
  import { listen } from "@tauri-apps/api/event";
  import ManageAddOnsModal from "$lib/components/ManageAddOnsModal.svelte";
  let showAddOnsModal = false;
  let selectedModName = "";
  let selectedAddOns: InstalledModFile[] = [];
  let addonMap: Record<string, string[]> = {};
  function openAddOnsModal(modName: string) {
    selectedModName = modName;
    selectedAddOns = getAddOnsForMod(modName);
    showAddOnsModal = true;
  }

  function closeAddOnsModal() {
    showAddOnsModal = false;
    selectedModName = "";
    selectedAddOns = [];
    refresh();
  }

  async function handleAddAddOns(event: CustomEvent) {
    const files = event.detail.files || [];
    const newAddOns = files.map((f: any) => ({
      name: typeof f === "string" ? f.split(/[\\/]/).pop() : f.name,
      path: typeof f === "string" ? f : f.path || f.name,
      exists: true,
    }));

    // Install add-on files
    for (const addOn of newAddOns) {
      try {
        await installLocalMod(addOn.path);
        toastStore.success(`Installed add-on: ${addOn.name}`);
      } catch (e) {
        toastStore.error(`Failed to install add-on: ${addOn.name}`);
      }
    }

    // Link the new addon archives to the parent in the addon_map
    const newArchiveNames = newAddOns.map((a: { name: string }) => a.name);
    addonMap[selectedModName] = [
      ...new Set([...(addonMap[selectedModName] ?? []), ...newArchiveNames]),
    ];
    await setAddonMap(addonMap);

    // Refresh - backend repopulates addonFiles and modsForActiveProfile is
    // reloaded from disk (installLocalMod added each addon to the profile).
    await refresh();
    selectedAddOns = getAddOnsForMod(selectedModName);

    // installLocalMod adds each addon to the active profile directly, but addons
    // must only be linked via their parent. Strip all known addon archives from
    // the profile now that addonMap is fresh (set by refresh()).
    const allAddonArchives = new Set(Object.values(addonMap).flat());
    const cleanedProfile = modsForActiveProfile.filter(
      (n) => !allAddonArchives.has(n),
    );
    if (cleanedProfile.length !== modsForActiveProfile.length) {
      await persistActiveProfileEnabledGroups(cleanedProfile);
      modsForActiveProfile = cleanedProfile;
    }

    // Sync: creates addon symlinks only if the parent is enabled.
    if (hasGamePath) {
      await syncModLinks(getFullSyncList(modsForActiveProfile));
    }
  }

  async function handleRemoveAddOn(event: CustomEvent) {
    const idx = event.detail.index;
    if (idx < 0) return;

    const removedAddOn = selectedAddOns[idx];
    selectedAddOns = selectedAddOns
      .slice(0, idx)
      .concat(selectedAddOns.slice(idx + 1));

    // Update addon_map: remove by archive name if available, else by file name
    const archiveName = removedAddOn?.archiveName;
    if (archiveName) {
      const updated = (addonMap[selectedModName] ?? []).filter(
        (n) => n !== archiveName,
      );
      if (updated.length === 0) {
        delete addonMap[selectedModName];
      } else {
        addonMap[selectedModName] = updated;
      }
      await setAddonMap(addonMap);
      try {
        await uninstallArchive(archiveName);
        toastStore.success(`Removed add-on: ${removedAddOn.name}`);
      } catch (e) {
        toastStore.error(`Failed to remove add-on: ${removedAddOn.name}`);
      }
    } else if (removedAddOn) {
      try {
        await uninstallMod(removedAddOn.name);
        toastStore.success(`Removed add-on: ${removedAddOn.name}`);
      } catch (e) {
        toastStore.error(`Failed to remove add-on: ${removedAddOn.name}`);
      }
    }

    await refresh();
    selectedAddOns = getAddOnsForMod(selectedModName);

    if (hasGamePath) {
      await syncModLinks(getFullSyncList(modsForActiveProfile));
    }
  }
  import ItemPickerModal from "$lib/components/ItemPickerModal.svelte";
  import BrokenModModal from "$lib/components/BrokenModModal.svelte";
  import ConfirmModal from "$lib/components/ConfirmModal.svelte";
  import { Menu, MenuItem } from "@tauri-apps/api/menu";
  import SourceIcon from "$lib/components/SourceIcon.svelte";
  import { modAddQueueStore } from "$lib/stores/modAddQueue";
  import { modSortOrder } from "$lib/stores/modSortOrder";
  import { showBroken } from "$lib/stores/showBroken";
  import { toastStore } from "$lib/stores/toast";
  import { formatDistanceToNow } from "date-fns";
  import type {
    InstalledModGroup,
    InstalledModFile,
    Profile,
  } from "$lib/types";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import {
    AlertTriangle,
    Calendar,
    ExternalLink,
    Globe,
    Layers,
    Library,
    Link as LinkIcon,
    Pencil,
    Plus,
    Shield,
    Tag,
    Trash2,
  } from "lucide-svelte";
  import { onMount } from "svelte";

  let modGroups: (InstalledModGroup & { addonFiles?: InstalledModFile[] })[] =
    [];
  // Add-on helpers
  function getAddOnsForMod(modName: string): InstalledModFile[] {
    return modGroups.find((g) => g.name === modName)?.addonFiles ?? [];
  }

  // Returns enabled archive names plus addon archive names for each enabled mod,
  // so syncModLinks also creates/removes addon symlinks correctly.
  function getFullSyncList(enabledGroups: string[]): string[] {
    const enabled = new Set(enabledGroups);
    const addonArchives = modGroups
      .filter((g) => enabled.has(g.name))
      .flatMap((g) =>
        (g.addonFiles ?? [])
          .map((f) => f.archiveName)
          .filter((n): n is string => !!n),
      );
    return [...new Set([...enabledGroups, ...addonArchives])];
  }
  let brokenModsMap: Record<string, string> = {};
  let noWorldGenSet: Set<string> = new Set();
  let showBrokenModal = false;
  let brokenModalModName = "";
  let brokenModalModLabel = "";
  let modSearch = "";
  let modSourceFilter: "all" | "nexus" | "modio" = "all";
  let modsForActiveProfile: string[] = [];
  let showAddModModal = false;
  let prevDoneCounter = $addModpackPanelStore.doneCounter;
  $: if ($addModpackPanelStore.doneCounter !== prevDoneCounter) {
    prevDoneCounter = $addModpackPanelStore.doneCounter;
    void refresh();
  }
  let prevModInstalledCounter = $addModpackPanelStore.modInstalledCounter;
  $: if (
    $addModpackPanelStore.modInstalledCounter !== prevModInstalledCounter
  ) {
    prevModInstalledCounter = $addModpackPanelStore.modInstalledCounter;
    void refreshModList();
  }
  let nexusFreeDownloads: Array<{
    prettyName: string | null;
    fileName: string;
    modUrl: string;
  }> = [];
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
  let isProcessingFiles = false;
  const pendingFileQueue: Array<{
    path: string;
    fileName: string;
    queueId: string;
  }> = [];
  let editingGroup: string | null = null;
  let editInputValue = "";
  let editingUrlGroup: string | null = null;
  let editUrlInputValue = "";
  let showCollectionPickerModal = false;
  let collectionPickerModName = "";
  let collectionPickerModLabel = "";
  let activeCollectionNames: string[] = [];
  let activeProfileCollections: Record<string, string[]> = {};
  let activeProfileCollectionColors: Record<string, string> = {};

  let showTagPickerModal = false;
  let tagPickerModName = "";
  let tagPickerModLabel = "";
  let activeProfileTags: Record<string, string[]> = {};
  let allTagNames: string[] = [];
  let activeTagFilters = new Set<string>();
  let activeCollectionFilters = new Set<string>();
  let selectedMods = new Set<string>();
  let showBulkCollectionModal = false;
  let showBulkTagModal = false;

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
  // Filter out add-on files that are tracked as standalone mods

  $: filteredModGroups = modGroups
    .filter((group) => {
      const search = modSearch.trim().toLowerCase();
      const name = group.name.toLowerCase();
      const displayName = (group.displayName || "").toLowerCase();
      const matchesSearch =
        !search || name.includes(search) || displayName.includes(search);
      const source = getModSource(group.sourceUrl);
      const matchesSource =
        modSourceFilter === "all" || source === modSourceFilter;
      const matchesTags =
        activeTagFilters.size === 0 ||
        (modToTagsMap[group.name] ?? []).some((t) => activeTagFilters.has(t));
      const matchesCollections =
        activeCollectionFilters.size === 0 ||
        (modToCollectionsMap[group.name] ?? []).some((c) =>
          activeCollectionFilters.has(c),
        );
      const isBroken = brokenModsMap[group.name] !== undefined;
      const matchesBroken = $showBroken || !isBroken;
      return (
        matchesSearch &&
        matchesSource &&
        matchesTags &&
        matchesCollections &&
        matchesBroken
      );
    })
    .sort((a, b) => {
      const labelA = (a.displayName || a.name).toLowerCase();
      const labelB = (b.displayName || b.name).toLowerCase();
      switch ($modSortOrder) {
        case "alpha-asc":
          return labelA.localeCompare(labelB);
        case "alpha-desc":
          return labelB.localeCompare(labelA);
        case "date-asc":
          return (a.installedAt ?? 0) - (b.installedAt ?? 0);
        case "date-desc":
          return (b.installedAt ?? 0) - (a.installedAt ?? 0);
        case "files-desc": {
          const aCount = a.files.length + (a.addonFiles?.length ?? 0);
          const bCount = b.files.length + (b.addonFiles?.length ?? 0);
          if (bCount !== aCount) return bCount - aCount;
          return labelA.localeCompare(labelB);
        }
        case "files-asc": {
          const aCount = a.files.length + (a.addonFiles?.length ?? 0);
          const bCount = b.files.length + (b.addonFiles?.length ?? 0);
          if (aCount !== bCount) return aCount - bCount;
          return labelA.localeCompare(labelB);
        }
        case "missing-sav-first": {
          const aNeeds = missingSavMapMods.has(a.name) ? 0 : 1;
          const bNeeds = missingSavMapMods.has(b.name) ? 0 : 1;
          if (aNeeds !== bNeeds) return aNeeds - bNeeds;
          return labelA.localeCompare(labelB);
        }
      }
    });

  $: modToCollectionsMap = Object.entries(activeProfileCollections).reduce(
    (acc, [colName, mods]) => {
      for (const mod of mods) {
        (acc[mod] ??= []).push(colName);
      }
      return acc;
    },
    {} as Record<string, string[]>,
  );

  $: modToTagsMap = Object.entries(activeProfileTags).reduce(
    (acc, [tagName, mods]) => {
      for (const mod of mods) {
        (acc[mod] ??= []).push(tagName);
      }
      return acc;
    },
    {} as Record<string, string[]>,
  );

  $: missingSavMapMods = new Set(
    modGroups
      .filter((group) => {
        if (noWorldGenSet.has(group.name)) return false;
        const tags = modToTagsMap[group.name] ?? [];
        if (!tags.some((t) => t.toLowerCase() === "map")) return false;
        const allFiles = [...group.files, ...(group.addonFiles ?? [])];
        return !allFiles.some((f) => f.name.toLowerCase().endsWith(".sav"));
      })
      .map((g) => g.name),
  );

  $: bulkCurrentCollections = activeCollectionNames.filter(
    (col) =>
      selectedMods.size > 0 &&
      [...selectedMods].every((m) =>
        (activeProfileCollections[col] ?? []).includes(m),
      ),
  );

  $: bulkPartialCollections = activeCollectionNames.filter((col) => {
    const mods = activeProfileCollections[col] ?? [];
    return (
      [...selectedMods].some((m) => mods.includes(m)) &&
      !bulkCurrentCollections.includes(col)
    );
  });

  $: bulkCurrentTags = allTagNames.filter(
    (tag) =>
      selectedMods.size > 0 &&
      [...selectedMods].every((m) =>
        (activeProfileTags[tag] ?? []).includes(m),
      ),
  );

  $: bulkPartialTags = allTagNames.filter((tag) => {
    const mods = activeProfileTags[tag] ?? [];
    return (
      [...selectedMods].some((m) => mods.includes(m)) &&
      !bulkCurrentTags.includes(tag)
    );
  });

  // Remove mods from selection when they're filtered out of view
  $: {
    const visible = new Set(filteredModGroups.map((g) => g.name));
    for (const m of selectedMods) if (!visible.has(m)) selectedMods.delete(m);
    selectedMods = selectedMods;
  }

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
      const aHasUrl = a.sourceUrl ? 1 : 0;
      const bHasUrl = b.sourceUrl ? 1 : 0;
      if (aHasUrl !== bHasUrl) return aHasUrl - bHasUrl;
      const aName = (a.displayName || a.name).toLowerCase();
      const bName = (b.displayName || b.name).toLowerCase();
      return aName.localeCompare(bName);
    });
  }

  async function refreshModList() {
    try {
      const groups = await getInstalledModGroups();
      allInstalledGroupNames = new Set(groups.map((g) => g.name));
      modGroups = sortModGroups(groups);
      expandedGroups = Object.fromEntries(
        modGroups.map((group) => [
          group.name,
          expandedGroups[group.name] ?? false,
        ]),
      );
    } catch {
      // silently fail - full refresh() at end will catch it
    }
  }

  async function refresh() {
    try {
      const [groups, config, profileList, map, broken, noWorldGen] =
        await Promise.all([
          getInstalledModGroups(),
          getConfig(),
          listProfiles().catch(() => []),
          getAddonMap(),
          getBrokenMods().catch(() => ({}) as Record<string, string>),
          getNoWorldGenMods().catch(() => [] as string[]),
        ]);
      brokenModsMap = broken;
      noWorldGenSet = new Set(noWorldGen);

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
          activeProfileCollections = activeProfile.collections ?? {};
          activeProfileCollectionColors = activeProfile.collection_colors ?? {};
          activeCollectionNames = Object.keys(activeProfileCollections);
          activeProfileTags = activeProfile.tags ?? {};
          allTagNames = Object.keys(activeProfileTags).sort((a, b) =>
            a.localeCompare(b),
          );
        } else {
          activeCollectionNames = [];
          activeProfileTags = {};
          allTagNames = [];
        }
      } else {
        activeCollectionNames = [];
        activeProfileTags = {};
        allTagNames = [];
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

      // Update add-on map
      addonMap = map;

      // Backend already filters addon groups and populates addonFiles on parents
      modGroups = sortModGroups(groups);
      hasGamePath = config.game_path != null;
      profiles = await ensureDefaultProfile(profileList);
      expandedGroups = Object.fromEntries(
        modGroups.map((group) => [
          group.name,
          expandedGroups[group.name] ?? false,
        ]),
      );

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
      clearSelection();
      const profile = await applyProfile(selectedProfile);

      await refresh();

      if (hasGamePath) {
        const enabledGroups = [...profile.installed_mod_names];
        await syncModLinks(getFullSyncList(enabledGroups));
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

        const cfg = await getConfig();
        const appWindow = getCurrentWindow();
        if (cfg.on_game_launch === "minimize") {
          if (cfg.minimize_target === "tray") {
            await appWindow.hide();
          } else {
            await appWindow.minimize();
          }
        } else if (cfg.on_game_launch === "close") {
          window.dispatchEvent(new CustomEvent("ron:launch-close"));
          await appWindow.close();
        }
        return;
      }
    } catch (error) {
      toastStore.error(`Failed to launch game: ${String(error)}`);
    }
  }

  function toggleGroup(name: string) {
    expandedGroups[name] = !expandedGroups[name];
  }

  function toggleModSelection(name: string) {
    if (selectedMods.has(name)) selectedMods.delete(name);
    else selectedMods.add(name);
    selectedMods = selectedMods;
  }

  function clearSelection() {
    selectedMods = new Set();
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
      const group = modGroups.find((g) => g.name === name);
      const addOns = group?.addonFiles || [];

      if (currentMods.has(name)) {
        currentMods.delete(name);
      } else {
        currentMods.add(name);
      }

      const enabledGroups = Array.from(currentMods);

      // Persist the update
      await persistActiveProfileEnabledGroups(enabledGroups);
      modsForActiveProfile = enabledGroups;

      // Sync game links - includes addon archives for all enabled mods
      if (hasGamePath) {
        await syncModLinks(getFullSyncList(enabledGroups));
      }
    } catch (error) {
      toastStore.error(`Failed to update active mods: ${String(error)}`);
    }
  }

  function openCollectionPicker(modName: string, modLabel: string) {
    if (!activeProfileName) {
      toastStore.error("Select an active profile first.");
      return;
    }

    collectionPickerModName = modName;
    collectionPickerModLabel = modLabel;
    showCollectionPickerModal = true;
  }

  async function handleCollectionToggle(
    event: CustomEvent<{ itemName: string }>,
  ) {
    const name = event.detail.itemName;
    const modName = collectionPickerModName;
    const currentCols = modToCollectionsMap[modName] ?? [];
    try {
      if (currentCols.includes(name)) {
        await removeModFromCollection(name, modName);
      } else {
        await addModToCollection(name, modName);
      }
      window.dispatchEvent(new CustomEvent("ron:collections-changed"));
      await refresh();
    } catch (error) {
      toastStore.error(`Failed to update collection: ${String(error)}`);
    }
  }

  async function handleCollectionCreate(
    event: CustomEvent<{ itemName: string }>,
  ) {
    const name = event.detail.itemName;
    const modName = collectionPickerModName;
    const modLabel = collectionPickerModLabel;
    try {
      await createCollection(name, [modName]);
      toastStore.success(`Created "${name}" and added ${modLabel || modName}.`);
      window.dispatchEvent(new CustomEvent("ron:collections-changed"));
      await refresh();
    } catch (error) {
      toastStore.error(`Failed to create collection: ${String(error)}`);
    }
  }

  function openTagPicker(modName: string, modLabel: string) {
    if (!activeProfileName) {
      toastStore.error("Select an active profile first.");
      return;
    }
    tagPickerModName = modName;
    tagPickerModLabel = modLabel;
    showTagPickerModal = true;
  }

  async function handleTagToggle(event: CustomEvent<{ itemName: string }>) {
    const tag = event.detail.itemName;
    const modName = tagPickerModName;
    const currentTags = modToTagsMap[modName] ?? [];
    const newTags = currentTags.includes(tag)
      ? currentTags.filter((t) => t !== tag)
      : [...currentTags, tag];
    try {
      await setModTags(modName, newTags);
      await refresh();
    } catch (error) {
      toastStore.error(`Failed to update tags: ${String(error)}`);
    }
  }

  async function handleTagCreate(event: CustomEvent<{ itemName: string }>) {
    const tag = event.detail.itemName;
    const modName = tagPickerModName;
    const currentTags = modToTagsMap[modName] ?? [];
    try {
      await setModTags(modName, [...new Set([...currentTags, tag])]);
      await refresh();
    } catch (error) {
      toastStore.error(`Failed to update tags: ${String(error)}`);
    }
  }

  async function handleTagDelete(event: CustomEvent<{ itemName: string }>) {
    try {
      await deleteTag(event.detail.itemName);
      await refresh();
    } catch (error) {
      toastStore.error(`Failed to delete tag: ${String(error)}`);
    }
  }

  async function handleBulkCollectionToggle(
    event: CustomEvent<{ itemName: string }>,
  ) {
    const col = event.detail.itemName;
    const inCol = activeProfileCollections[col] ?? [];
    const allHave = [...selectedMods].every((m) => inCol.includes(m));
    try {
      for (const mod of selectedMods) {
        if (allHave) await removeModFromCollection(col, mod);
        else if (!inCol.includes(mod)) await addModToCollection(col, mod);
      }
      window.dispatchEvent(new CustomEvent("ron:collections-changed"));
      await refresh();
    } catch (error) {
      toastStore.error(`Failed to update collection: ${String(error)}`);
    }
  }

  async function handleBulkCollectionCreate(
    event: CustomEvent<{ itemName: string }>,
  ) {
    const name = event.detail.itemName;
    try {
      await createCollection(name, [...selectedMods]);
      toastStore.success(
        `Created "${name}" and added ${selectedMods.size} mod${selectedMods.size === 1 ? "" : "s"}.`,
      );
      window.dispatchEvent(new CustomEvent("ron:collections-changed"));
      await refresh();
    } catch (error) {
      toastStore.error(`Failed to create collection: ${String(error)}`);
    }
  }

  async function handleBulkTagToggle(event: CustomEvent<{ itemName: string }>) {
    const tag = event.detail.itemName;
    const withTag = activeProfileTags[tag] ?? [];
    const allHave = [...selectedMods].every((m) => withTag.includes(m));
    try {
      for (const mod of selectedMods) {
        const cur = modToTagsMap[mod] ?? [];
        await setModTags(
          mod,
          allHave ? cur.filter((t) => t !== tag) : [...new Set([...cur, tag])],
        );
      }
      await refresh();
    } catch (error) {
      toastStore.error(`Failed to update tags: ${String(error)}`);
    }
  }

  async function handleBulkTagCreate(event: CustomEvent<{ itemName: string }>) {
    const tag = event.detail.itemName;
    try {
      for (const mod of selectedMods) {
        const cur = modToTagsMap[mod] ?? [];
        await setModTags(mod, [...new Set([...cur, tag])]);
      }
      toastStore.success(
        `Created tag "${tag}" and applied to ${selectedMods.size} mod${selectedMods.size === 1 ? "" : "s"}.`,
      );
      await refresh();
    } catch (error) {
      toastStore.error(`Failed to create tag: ${String(error)}`);
    }
  }

  async function handleUninstallMod(filename: string) {
    try {
      await uninstallMod(filename);
      toastStore.success(`Uninstalled: ${filename}`);

      await refresh();
      if (hasGamePath) {
        await syncModLinks(getFullSyncList(modsForActiveProfile));
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
            await syncModLinks(getFullSyncList(modsForActiveProfile));
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

  async function handleUninstallSelected() {
    const count = selectedMods.size;
    confirmModal = {
      isVisible: true,
      title: `Uninstall ${count} mod${count === 1 ? "" : "s"}?`,
      message: `Are you sure you want to uninstall ${count} selected mod${count === 1 ? "" : "s"}? This cannot be undone.`,
      confirmLabel: "Uninstall",
      detail: "",
      onConfirm: async () => {
        try {
          for (const modName of selectedMods) {
            const group = modGroups.find((g) => g.name === modName);
            if (!group) continue;
            if (group.managedByManifest) {
              await uninstallArchive(group.name);
            } else {
              const primaryFile = group.files[0]?.name;
              if (primaryFile) {
                await uninstallMod(primaryFile);
              }
            }
          }
          toastStore.success(
            `Uninstalled ${count} mod${count === 1 ? "" : "s"}`,
          );
          clearSelection();
          await refresh();
          if (hasGamePath) {
            await syncModLinks(getFullSyncList(modsForActiveProfile));
          }
        } catch (error) {
          toastStore.error(`Failed to uninstall: ${String(error)}`);
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
        await syncModLinks(getFullSyncList(enabledGroups));
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

    // Enqueue all dropped files - totalQueued accumulates correctly across drops
    for (const filePath of paths) {
      const fileName = filePath.split(/[\\/]/).pop() || filePath;
      pendingFileQueue.push({
        path: filePath,
        fileName,
        queueId: modAddQueueStore.enqueue(fileName),
      });
    }

    // Single worker loop - if already running, the new items will be picked up naturally
    if (isProcessingFiles) return;

    isProcessingFiles = true;
    try {
      while (pendingFileQueue.length > 0) {
        const entry = pendingFileQueue.shift()!;
        console.log("Installing file:", entry.path);
        try {
          let selectedPaks: string[] | undefined;
          const ext = entry.path.split(".").pop()?.toLowerCase() ?? "";
          if (ext === "zip" || ext === "rar" || ext === "7z") {
            try {
              const paks = await getArchivePakFiles(entry.path);
              if (paks.length > 1) {
                modAddQueueStore.markRunning(
                  entry.queueId,
                  "Select PAK files to install...",
                );
                const selected = await requestPakSelection(
                  entry.fileName,
                  paks,
                );
                if (selected === null) {
                  modAddQueueStore.markError(entry.queueId, "Cancelled");
                  continue;
                }
                selectedPaks = selected;
              }
            } catch {
              // fall through - install all
            }
          }
          modAddQueueStore.markRunning(
            entry.queueId,
            `Installing ${entry.fileName}...`,
          );
          await installLocalMod(entry.path, selectedPaks);
          modAddQueueStore.markDone(
            entry.queueId,
            `Installed ${entry.fileName}`,
          );
          toastStore.success(`Successfully installed: ${entry.fileName}`);
          addModpackPanelStore.notifyModInstalled();
          console.log("Installation successful:", entry.fileName);
        } catch (error) {
          const message = `Failed to install ${entry.fileName}: ${String(error)}`;
          modAddQueueStore.markError(entry.queueId, message);
          toastStore.error(message);
          console.error("Installation failed:", error);
        }
      }
    } finally {
      isProcessingFiles = false;
      await refresh();
    }
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

    let unlistenFreeDownload: (() => void) | null = null;
    void listen<{
      prettyName: string | null;
      fileName: string;
      modUrl: string;
    }>("nexus_free_download_waiting", (event) => {
      if (
        !nexusFreeDownloads.some((d) => d.fileName === event.payload.fileName)
      ) {
        nexusFreeDownloads = [...nexusFreeDownloads, event.payload];
      }
    }).then((fn) => {
      unlistenFreeDownload = fn;
    });

    let unlistenFreeDownloadComplete: (() => void) | null = null;
    void listen<{ fileName: string }>(
      "nexus_free_download_complete",
      (event) => {
        nexusFreeDownloads = nexusFreeDownloads.filter(
          (d) => d.fileName !== event.payload.fileName,
        );
      },
    ).then((fn) => {
      unlistenFreeDownloadComplete = fn;
    });

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

      if (unlistenFreeDownload) {
        unlistenFreeDownload();
      }
      if (unlistenFreeDownloadComplete) {
        unlistenFreeDownloadComplete();
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

<ItemPickerModal
  isVisible={showCollectionPickerModal}
  modLabel={collectionPickerModLabel || collectionPickerModName}
  allItems={activeCollectionNames}
  currentItems={modToCollectionsMap[collectionPickerModName] ?? []}
  title="Collections"
  ItemIcon={Library}
  accentColorVar="--clr-primary-300"
  searchPlaceholder="Search collections or type new name"
  createButtonText={(n) => `Create "${n}" and add mod`}
  allowDelete={false}
  noteText="To delete a collection, visit the Collections page."
  on:close={() => {
    showCollectionPickerModal = false;
    collectionPickerModName = "";
    collectionPickerModLabel = "";
  }}
  on:toggle={handleCollectionToggle}
  on:create={handleCollectionCreate}
/>

<ItemPickerModal
  isVisible={showTagPickerModal}
  modLabel={tagPickerModLabel || tagPickerModName}
  allItems={allTagNames}
  currentItems={modToTagsMap[tagPickerModName] ?? []}
  title="Tags"
  ItemIcon={Tag}
  accentColorVar="--clr-success-300"
  searchPlaceholder="Search tags or type new name"
  createButtonText={(n) => `Create "${n}" and select`}
  allowDelete={true}
  on:close={() => {
    showTagPickerModal = false;
    tagPickerModName = "";
    tagPickerModLabel = "";
  }}
  on:toggle={handleTagToggle}
  on:create={handleTagCreate}
  on:deleteItem={handleTagDelete}
/>

<ItemPickerModal
  isVisible={showBulkCollectionModal}
  subtitle="{selectedMods.size} mod{selectedMods.size === 1
    ? ''
    : 's'} selected"
  modLabel=""
  allItems={activeCollectionNames}
  currentItems={bulkCurrentCollections}
  partialItems={bulkPartialCollections}
  title="Collections"
  ItemIcon={Library}
  accentColorVar="--clr-primary-300"
  searchPlaceholder="Search collections or type new name"
  createButtonText={(n) => `Create "${n}" and add to all selected mods`}
  allowDelete={false}
  noteText="To delete a collection, visit the Collections page."
  on:close={() => {
    showBulkCollectionModal = false;
  }}
  on:toggle={handleBulkCollectionToggle}
  on:create={handleBulkCollectionCreate}
/>

<ItemPickerModal
  isVisible={showBulkTagModal}
  subtitle="{selectedMods.size} mod{selectedMods.size === 1
    ? ''
    : 's'} selected"
  modLabel=""
  allItems={allTagNames}
  currentItems={bulkCurrentTags}
  partialItems={bulkPartialTags}
  title="Tags"
  ItemIcon={Tag}
  accentColorVar="--clr-success-300"
  searchPlaceholder="Search tags or type new name"
  createButtonText={(n) => `Create "${n}" and apply to all selected mods`}
  allowDelete={true}
  on:close={() => {
    showBulkTagModal = false;
  }}
  on:toggle={handleBulkTagToggle}
  on:create={handleBulkTagCreate}
  on:deleteItem={handleTagDelete}
/>

{#if $pakSelectionStore}
  <PakFileSelectionModal
    archiveName={$pakSelectionStore.archiveName}
    paks={$pakSelectionStore.paks}
    on:select={(e) => {
      $pakSelectionStore?.resolve(e.detail.selected);
      pakSelectionStore.set(null);
    }}
    on:cancel={() => {
      $pakSelectionStore?.resolve(null);
      pakSelectionStore.set(null);
    }}
  />
{/if}

{#if $nexusFileSelectionStore}
  <NexusFileSelectionModal
    modName={$nexusFileSelectionStore.modName}
    files={$nexusFileSelectionStore.files}
    on:select={(e) => {
      $nexusFileSelectionStore?.resolve(e.detail);
      nexusFileSelectionStore.set(null);
    }}
    on:cancel={() => {
      $nexusFileSelectionStore?.resolve(null);
      nexusFileSelectionStore.set(null);
    }}
  />
{/if}

{#if nexusFreeDownloads.length > 0}
  <NexusFreeDownloadModal
    downloads={nexusFreeDownloads}
    on:cancel={() => {
      nexusFreeDownloads = [];
    }}
  />
{/if}

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
    style="width: 140px; padding-top: 0; padding-bottom: 0; height: 2.5rem;"
    aria-label="Filter by source"
  >
    <option value="all">All Sources</option>
    <option value="modio">Mod.io</option>
    <option value="nexus">Nexus</option>
  </select>
  <select
    class="input"
    bind:value={$modSortOrder}
    style="width: 150px; padding-top: 0; padding-bottom: 0; height: 2.5rem;"
    aria-label="Sort order"
  >
    <option value="alpha-asc">A → Z</option>
    <option value="alpha-desc">Z → A</option>
    <option value="date-desc">Newest First</option>
    <option value="date-asc">Oldest First</option>
    <option value="files-desc">Most Files First</option>
    <option value="files-asc">Fewest Files First</option>
    <option value="missing-sav-first">Missing World Gen</option>
  </select>
  <button
    on:click={() => ($showBroken = !$showBroken)}
    style={$showBroken
      ? "background: color-mix(in srgb, var(--clr-danger-300) 15%, transparent); border-color: var(--clr-danger-300); color: var(--clr-danger-300);"
      : "border-color: var(--adw-border-color); color: var(--clr-text-secondary);"}
    class="inline-flex items-center gap-1.5 rounded border px-2 text-xs cursor-pointer"
    style:height="2.5rem"
    title={$showBroken
      ? "Broken mods visible - click to hide"
      : "Broken mods hidden - click to show"}
  >
    <AlertTriangle size={13} />
    {$showBroken ? "Showing broken" : "Show broken"}
  </button>
</div>

{#if allTagNames.length > 0}
  <div class="flex flex-wrap items-center gap-1.5 mb-3">
    <span
      style="color: var(--clr-text-secondary);"
      class="text-xs flex items-center gap-1 mr-1"
    >
      <Tag size={12} />
      Filter by tag:
    </span>
    {#each allTagNames as tagName (tagName)}
      <button
        on:click={() => {
          if (activeTagFilters.has(tagName)) {
            activeTagFilters.delete(tagName);
          } else {
            activeTagFilters.add(tagName);
          }
          activeTagFilters = activeTagFilters;
        }}
        style={activeTagFilters.has(tagName)
          ? "background: color-mix(in srgb, var(--clr-success-300) 20%, transparent); border-color: var(--clr-success-300); color: var(--clr-success-300);"
          : "border-color: var(--adw-border-color); color: var(--clr-text-secondary);"}
        class="inline-flex items-center gap-1 rounded border px-2 py-0.5 text-xs cursor-pointer"
      >
        <Tag size={10} />
        {tagName}
      </button>
    {/each}
    {#if activeTagFilters.size > 0}
      <button
        on:click={() => {
          activeTagFilters = new Set();
        }}
        style="color: var(--clr-text-secondary);"
        class="text-xs underline ml-1 cursor-pointer"
      >
        Clear
      </button>
    {/if}
  </div>
{/if}

{#if activeCollectionNames.length > 0}
  <div class="flex flex-wrap items-center gap-1.5 mb-3">
    <span
      style="color: var(--clr-text-secondary);"
      class="text-xs flex items-center gap-1 mr-1"
    >
      <Layers size={12} />
      Filter by collection:
    </span>
    {#each activeCollectionNames as col (col)}
      {@const colColor =
        activeProfileCollectionColors[col] ?? "var(--clr-accent-300)"}
      <button
        on:click={() => {
          if (activeCollectionFilters.has(col)) {
            activeCollectionFilters.delete(col);
          } else {
            activeCollectionFilters.add(col);
          }
          activeCollectionFilters = activeCollectionFilters;
        }}
        style={activeCollectionFilters.has(col)
          ? `background: color-mix(in srgb, ${colColor} 20%, transparent); border-color: ${colColor}; color: ${colColor};`
          : "border-color: var(--adw-border-color); color: var(--clr-text-secondary);"}
        class="inline-flex items-center gap-1 rounded border px-2 py-0.5 text-xs cursor-pointer"
      >
        <Layers size={10} />
        {col}
      </button>
    {/each}
    {#if activeCollectionFilters.size > 0}
      <button
        on:click={() => {
          activeCollectionFilters = new Set();
        }}
        style="color: var(--clr-text-secondary);"
        class="text-xs underline ml-1 cursor-pointer"
      >
        Clear
      </button>
    {/if}
  </div>
{/if}

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
        on:click={() => {
          addModpackPanelStore.open("add");
        }}
        class="btn btn-sm btn-primary"
        title="Add Modpack"
      >
        <Globe size={16} class="inline mr-1" />
        Add Modpack
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

  {#if selectedMods.size > 0}
    <div
      style="background: color-mix(in srgb, var(--clr-primary-300) 10%, var(--clr-surface-variant)); border-color: var(--clr-primary-300);"
      class="flex items-center gap-3 px-3 py-2 rounded border mb-2 text-sm"
    >
      <span style="color: var(--clr-text);" class="font-medium flex-1">
        {selectedMods.size} mod{selectedMods.size === 1 ? "" : "s"} selected
      </span>
      <button
        class="btn btn-sm btn-primary"
        on:click={() => {
          showBulkCollectionModal = true;
        }}
        disabled={!activeProfileName}
      >
        <Library size={14} class="inline mr-1" />
        Manage Collections
      </button>
      <button
        class="btn btn-sm btn-success"
        on:click={() => {
          showBulkTagModal = true;
        }}
        disabled={!activeProfileName}
      >
        <Tag size={14} class="inline mr-1" />
        Manage Tags
      </button>
      <button
        class="btn btn-sm btn-danger"
        on:click={() => {
          void handleUninstallSelected();
        }}
      >
        <Trash2 size={14} class="inline mr-1" />
        Uninstall
      </button>
      <button class="btn btn-sm" on:click={clearSelection}>Clear</button>
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
          class="rounded border group/row"
          on:contextmenu|preventDefault={async () => {
            const menu = await Menu.new({
              items: [
                await MenuItem.new({
                  text: "Manage collections",
                  action: () =>
                    openCollectionPicker(
                      group.name,
                      group.displayName || group.name,
                    ),
                }),
                await MenuItem.new({
                  text: "Manage tags",
                  action: () =>
                    openTagPicker(group.name, group.displayName || group.name),
                }),
                await MenuItem.new({
                  text:
                    brokenModsMap[group.name] !== undefined
                      ? "Edit broken note"
                      : "Mark as broken",
                  action: () => {
                    brokenModalModName = group.name;
                    brokenModalModLabel = group.displayName || group.name;
                    showBrokenModal = true;
                  },
                }),
                await MenuItem.new({
                  text: "Manage add-ons",
                  action: () => openAddOnsModal(group.name),
                }),
                await MenuItem.new({
                  text: group.sourceUrl ? "Edit link" : "Add link",
                  action: () => {
                    expandedGroups[group.name] = true;
                    startEditingSourceUrl(group);
                  },
                }),
              ],
            });
            await menu.popup();
          }}
        >
          <div class="flex items-center justify-between px-3 py-2 gap-3">
            <input
              type="checkbox"
              class="mod-select-checkbox flex-shrink-0 cursor-pointer"
              class:is-selection-active={selectedMods.size > 0}
              style="accent-color: var(--clr-primary-300); width: 15px; height: 15px;"
              checked={selectedMods.has(group.name)}
              on:change|stopPropagation={() => toggleModSelection(group.name)}
              aria-label="Select {group.displayName ?? group.name}"
            />
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
                    {#if missingSavMapMods.has(group.name)}
                      <span
                        title="No world generation data"
                        class="flex items-center"
                      >
                        <AlertTriangle
                          size={14}
                          style="color: #f59e0b; flex-shrink: 0;"
                        />
                      </span>
                    {/if}
                    {#if brokenModsMap[group.name] !== undefined}
                      <span
                        title={brokenModsMap[group.name] || "Marked as broken"}
                        class="flex items-center"
                        style="color: var(--clr-danger-300);"
                      >
                        <AlertTriangle size={14} style="flex-shrink: 0;" />
                      </span>
                    {/if}
                    {#if group.hasOverrideFiles}
                      <span
                        title="Contains game file overrides"
                        class="flex items-center"
                      >
                        <Shield
                          size={14}
                          style="color: #f59e0b; flex-shrink: 0;"
                        />
                      </span>
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
                  {group.files.length + (group.addonFiles?.length || 0)} installed
                  file{group.files.length + (group.addonFiles?.length || 0) ===
                  1
                    ? ""
                    : "s"}
                  {#if group.addonFiles?.length}
                    <span style="color: var(--clr-primary-300);">
                      (+{group.addonFiles.length} add-on)</span
                    >
                  {/if}
                  {#if group.installedAt}
                    <span class="mx-1">·</span><span
                      title={new Date(
                        group.installedAt * 1000,
                      ).toLocaleString()}
                      >added {formatDistanceToNow(
                        new Date(group.installedAt * 1000),
                        { addSuffix: true },
                      )}</span
                    >
                  {/if}
                </p>
              </div>
            </button>

            <div class="flex items-center gap-2">
              {#if (modToCollectionsMap[group.name] ?? []).length > 0 || (modToTagsMap[group.name] ?? []).length > 0}
                <div class="flex items-center gap-1 flex-wrap">
                  {#each modToCollectionsMap[group.name] ?? [] as col (col)}
                    {@const colColor =
                      activeProfileCollectionColors[col] ?? null}
                    {@const isColActive = activeCollectionFilters.has(col)}
                    <button
                      on:click|stopPropagation={() => {
                        if (isColActive) {
                          activeCollectionFilters.delete(col);
                        } else {
                          activeCollectionFilters.add(col);
                        }
                        activeCollectionFilters = activeCollectionFilters;
                      }}
                      style={colColor
                        ? `background: color-mix(in srgb, ${colColor} ${isColActive ? 20 : 12}%, transparent); border-color: color-mix(in srgb, ${colColor} ${isColActive ? 100 : 40}%, transparent); color: ${colColor};`
                        : "background: var(--clr-surface); border-color: var(--adw-border-color); color: var(--clr-text-secondary);"}
                      class="inline-flex items-center gap-1 rounded border px-1.5 py-0.5 text-xs leading-none cursor-pointer"
                      title="Filter by collection: {col}"
                    >
                      <Layers size={10} />
                      {col}
                    </button>
                  {/each}
                  {#each modToTagsMap[group.name] ?? [] as tag (tag)}
                    {@const isTagActive = activeTagFilters.has(tag)}
                    <button
                      on:click|stopPropagation={() => {
                        if (isTagActive) {
                          activeTagFilters.delete(tag);
                        } else {
                          activeTagFilters.add(tag);
                        }
                        activeTagFilters = activeTagFilters;
                      }}
                      style="background: color-mix(in srgb, var(--clr-success-300) {isTagActive
                        ? 20
                        : 12}%, transparent); border-color: color-mix(in srgb, var(--clr-success-300) {isTagActive
                        ? 100
                        : 40}%, transparent); color: var(--clr-success-300);"
                      class="inline-flex items-center gap-1 rounded border px-1.5 py-0.5 text-xs leading-none cursor-pointer"
                      title="Filter by tag: {tag}"
                    >
                      <Tag size={10} />
                      {tag}
                    </button>
                  {/each}
                </div>
              {/if}
              <label
                class="gale-switch"
                class:opacity-50={!!brokenModsMap[group.name]}
                title={brokenModsMap[group.name] !== undefined
                  ? `Broken: ${brokenModsMap[group.name] || "no note"}`
                  : `${modsForActiveProfile.includes(group.name) ? "Disable" : "Enable"} ${group.displayName ?? group.name}`}
              >
                <input
                  type="checkbox"
                  checked={modsForActiveProfile.includes(group.name)}
                  on:change={() => toggleGroupState(group.name)}
                  disabled={!activeProfileName || !!brokenModsMap[group.name]}
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
                    {group.sourceUrl ? "Edit Link" : "Add link"}
                  </button>
                  <button
                    class="btn btn-sm flex-shrink-0"
                    on:click={() => openAddOnsModal(group.name)}
                    title="Manage add-ons for this mod"
                  >
                    Manage Add-ons
                  </button>
                {/if}
              </div>

              {#if group.installedAt}
                <div class="flex items-center gap-2 text-xs mb-2">
                  <span
                    style="color: var(--clr-text-secondary);"
                    class="inline-flex items-center gap-1 flex-shrink-0"
                  >
                    <Calendar size={13} />
                    Added
                  </span>
                  <span style="color: var(--clr-text-secondary);">
                    {new Date(group.installedAt * 1000).toLocaleString()}
                  </span>
                </div>
              {/if}

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
              {#if group.addonFiles?.length}
                <div
                  class="mt-2 text-xs font-semibold"
                  style="color: var(--clr-primary-300);"
                >
                  Add-ons:
                </div>
                {#each group.addonFiles as file (file.path)}
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
              {/if}
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}

  <ManageAddOnsModal
    isVisible={showAddOnsModal}
    modName={selectedModName}
    displayName={modGroups.find((g) => g.name === selectedModName)
      ?.displayName || selectedModName}
    addOns={selectedAddOns}
    noWorldGen={noWorldGenSet.has(selectedModName)}
    on:close={closeAddOnsModal}
    on:addAddOns={handleAddAddOns}
    on:removeAddOn={handleRemoveAddOn}
    on:toggleNoWorldGen={async (e) => {
      try {
        if (e.detail.exempt) {
          await setModNoWorldGen(selectedModName);
        } else {
          await clearModNoWorldGen(selectedModName);
        }
        noWorldGenSet = new Set(await getNoWorldGenMods());
      } catch (err) {
        toastStore.error(`Failed to update world gen setting: ${String(err)}`);
      }
    }}
  />

  <BrokenModModal
    isVisible={showBrokenModal}
    modLabel={brokenModalModLabel}
    existingNote={brokenModsMap[brokenModalModName] ?? ""}
    isAlreadyBroken={brokenModsMap[brokenModalModName] !== undefined}
    on:close={() => (showBrokenModal = false)}
    on:save={async (e) => {
      try {
        await setModBroken(brokenModalModName, e.detail.note);
        showBrokenModal = false;
        await refresh();
      } catch (err) {
        toastStore.error(`Failed to mark mod as broken: ${String(err)}`);
      }
    }}
    on:clear={async () => {
      try {
        await clearModBroken(brokenModalModName);
        showBrokenModal = false;
        await refresh();
      } catch (err) {
        toastStore.error(`Failed to clear broken flag: ${String(err)}`);
      }
    }}
  />
</div>
