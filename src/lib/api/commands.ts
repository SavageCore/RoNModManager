import { invoke } from "@tauri-apps/api/core";
import type { AppConfig, Collection, ModInfo, ModPack } from "../types";

export const getConfig = () => invoke<AppConfig>("get_config");
export const setTheme = (theme: "light" | "dark" | "system") =>
  invoke<void>("set_theme", { theme });
export const applyIntroSkip = () => invoke<void>("apply_intro_skip");
export const isIntroSkipApplied = () =>
  invoke<boolean>("is_intro_skip_applied");

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
export const exportModpackToFile = (modpack: ModPack, path: string) =>
  invoke<void>("export_modpack_to_file", { modpack, path });

export const getCollections = () =>
  invoke<Record<string, boolean>>("get_collections");
export const toggleCollection = (name: string, enabled: boolean) =>
  invoke<void>("toggle_collection", { name, enabled });

export const getAuthStatus = () => invoke<boolean>("get_auth_status");
export const openModioLogin = () => invoke<void>("open_modio_login");
export const saveToken = (token: string) =>
  invoke<void>("save_token", { token });
export const validateToken = () => invoke<boolean>("validate_token");
export const logout = () => invoke<void>("logout");

export const getModList = () => invoke<ModInfo[]>("get_mod_list");
export const installMods = (enabledCollections?: string[]) =>
  invoke<void>("install_mods", { enabled_collections: enabledCollections });
export const uninstallMods = () => invoke<void>("uninstall_mods");
