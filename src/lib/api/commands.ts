// Read manifest for a given archive name (calls Tauri backend)
export const readManifestForArchive = (archiveName: string) =>
  invoke<any>("read_manifest_for_archive", { archiveName: archiveName });
// Get subscription status for a mod.io mod
export const getModioSubscriptionStatus = (args: {
  mod_id: string;
  oauth_token: string;
}) => invoke<string>("get_modio_subscription_status", { args });
// Subscribe to a mod.io mod by mod ID and OAuth token
export const modioSubscribe = (args: { mod_id: string; oauth_token: string }) =>
  invoke<string>("modio_subscribe", { args });
// Download a mod archive from a URL to the local archives folder
export const downloadModArchive = (url: string, filename: string) =>
  invoke<void>("download_mod_archive", { url, filename });
// Fetch modpack JSON via Tauri backend to avoid CORS
export const fetchModpackJson = (url: string) =>
  invoke<any>("fetch_modpack_json", { url });
// Check if a file exists
export const fileExists = (path: string) =>
  invoke<boolean>("file_exists", { path });
import { invoke } from "@tauri-apps/api/core";
// Get the archive root path from backend
export const getArchiveRootPath = () => invoke<string>("get_archive_root_path");
import type {
  AppConfig,
  Collection,
  InstalledModGroup,
  ModInfo,
  ModPack,
  Profile,
  UpdateInfo,
  WindowState,
} from "../types";

export const getConfig = () => invoke<AppConfig>("get_config");
export const setTheme = (theme: "light" | "dark" | "system") =>
  invoke<void>("set_theme", { theme });
export const applyIntroSkip = () => invoke<void>("apply_intro_skip");
export const undoIntroSkip = () => invoke<void>("undo_intro_skip");
export const isIntroSkipApplied = () =>
  invoke<boolean>("is_intro_skip_applied");
export const getIntroSkipIniPath = () =>
  invoke<string>("get_intro_skip_ini_path");

export const detectGamePath = () => invoke<string | null>("detect_game_path");
export const setGamePath = (path: string) =>
  invoke<void>("set_game_path", { path });

export const setModpackUrl = (url: string) =>
  invoke<void>("set_modpack_url", { url });
export const syncModpack = () => invoke<ModPack>("sync_modpack");
export const getModpackCollections = () =>
  invoke<Record<string, Collection>>("get_modpack_collections");
export const buildModpackFromInstalled = () =>
  invoke<ModPack>("build_modpack_from_installed");
export const exportModpackToFile = (modpack: ModPack, dirPath: string) =>
  invoke<void>("export_modpack_to_file", { modpack, dirPath });

export const getCollections = () =>
  invoke<Record<string, boolean>>("get_collections");
export const getCollectionMods = () =>
  invoke<Record<string, string[]>>("get_collection_mods");
export const createCollection = (name: string, modNames: string[] = []) =>
  invoke<void>("create_collection", { name, modNames });
export const addModToCollection = (collection: string, modName: string) =>
  invoke<void>("add_mod_to_collection", { collection, modName });
export const removeModFromCollection = (collection: string, modName: string) =>
  invoke<void>("remove_mod_from_collection", { collection, modName });
export const deleteCollection = (name: string) =>
  invoke<void>("delete_collection", { name });
export const toggleCollection = (name: string, enabled: boolean) =>
  invoke<void>("toggle_collection", { name, enabled });

export const getTags = () => invoke<Record<string, string[]>>("get_tags");
export const setModTags = (modName: string, newTags: string[]) =>
  invoke<void>("set_mod_tags", { modName, newTags });
export const deleteTag = (name: string) => invoke<void>("delete_tag", { name });

export const getAuthStatus = () => invoke<boolean>("get_auth_status");
export const openModioLogin = () => invoke<void>("open_modio_login");
export const saveToken = (token: string) =>
  invoke<void>("save_token", { token });
export const validateToken = () => invoke<boolean>("validate_token");
export const logout = () => invoke<void>("logout");

export const getModList = () => invoke<ModInfo[]>("get_mod_list");
export const getInstalledModGroups = () =>
  invoke<InstalledModGroup[]>("get_installed_mod_groups");
export const installMods = (enabledCollections?: string[]) =>
  invoke<void>("install_mods", { enabled_collections: enabledCollections });
export const installLocalMod = (filePath: string) =>
  invoke<{ wasDuplicate: boolean }>("install_local_mod", { filePath });
export const addModIoMod = (input: string) =>
  invoke<{
    modId: number;
    name: string;
    archiveName: string;
    sourceUrl: string;
  }>("add_modio_mod", { input });
export const fetchNexusModInfo = (input: string) =>
  invoke<{
    modId: number;
    name: string;
    summary: string | null;
    modUrl: string;
  }>("fetch_nexus_mod_info", { input });
export interface ModioRemoteInfo {
  remote_md5: string | null;
  archive_name: string;
}
export const fetchModioRemoteInfo = (input: string): Promise<ModioRemoteInfo> =>
  invoke<ModioRemoteInfo>("get_modio_remote_info", { input });
export const refreshModMetadata = () =>
  invoke<{
    checked: number;
    refreshed: number;
    skipped: number;
    failed: number;
  }>("refresh_mod_metadata");

export const updateConfig = (updates: {
  nexus_api_key?: string | null;
  modio_api_key?: string | null;
  active_profile?: string | null;
  modpack_url?: string | null;
  modpack_version?: string | null;
}) => invoke<void>("update_config", { updates });

export const verifyNexusApiKey = (apiKey: string) =>
  invoke<boolean>("verify_nexus_api_key", { apiKey });

export const uninstallMods = () => invoke<void>("uninstall_mods");
export const uninstallMod = (filename: string) =>
  invoke<void>("uninstall_mod", { filename });
export const uninstallArchive = (archiveName: string) =>
  invoke<void>("uninstall_archive", { archiveName });
export const updateModDisplayName = (
  archiveName: string,
  displayName: string,
) => invoke<void>("update_mod_display_name", { archiveName, displayName });
export const updateModSourceUrl = (archiveName: string, sourceUrl: string) =>
  invoke<void>("update_mod_source_url", { archiveName, sourceUrl });

export const listProfiles = () => invoke<Profile[]>("list_profiles");
export const getProfile = (name: string) =>
  invoke<Profile | null>("get_profile", { name });
export const saveProfile = (
  name: string,
  description: string | null,
  installedModNames: string[],
) =>
  invoke<Profile>("save_profile", {
    name,
    description,
    installedModNames,
  });
export const deleteProfile = (name: string) =>
  invoke<void>("delete_profile", { name });
export const applyProfile = (name: string) =>
  invoke<Profile>("apply_profile", { name });

export const launchGame = () => invoke<void>("launch_game");
export const syncModLinks = (enabledGroups: string[]) =>
  invoke<void>("sync_mod_links", { enabledGroups });
export const launchGameWithGroups = (enabledGroups: string[]) =>
  invoke<void>("launch_game_with_groups", { enabledGroups });

export const setWindowTitle = (title: string) =>
  invoke<void>("set_window_title", { title });
export const saveWindowState = (
  width?: number,
  height?: number,
  x?: number,
  y?: number,
) =>
  invoke<void>("save_window_state", {
    width,
    height,
    x,
    y,
  });
export const getWindowState = () => invoke<WindowState>("get_window_state");

export const checkForUpdate = () => invoke<UpdateInfo>("check_for_update");
export const installUpdate = () => invoke<UpdateInfo>("install_update");

export const getAddonMap = () =>
  invoke<Record<string, string[]>>("get_addon_map");
export const setAddonMap = (map: Record<string, string[]>) =>
  invoke<void>("set_addon_map", { map });
