export type ThemeMode = "light" | "dark" | "system";
export type OnGameLaunchAction = "nothing" | "minimize" | "close";
export type CloseAction = "quit" | "minimize";
export type MinimizeTarget = "taskbar" | "tray";

export interface AppConfig {
  game_path: string | null;
  modpack_url: string | null;
  modpack_version: string | null;
  oauth_token: string | null;
  modio_api_key: string | null;
  modio_game_id: number | null;
  nexus_api_key: string | null;
  active_profile: string | null;
  theme: ThemeMode;
  window_width: number | null;
  window_height: number | null;
  window_x: number | null;
  window_y: number | null;
  last_update_check: string | null;
  sync_remote_host: string | null;
  sync_remote_path: string | null;
  on_game_launch: OnGameLaunchAction;
  close_action: CloseAction;
  minimize_target: MinimizeTarget;
  asked_close_preference: boolean;
}

export interface Collection {
  default_enabled: boolean;
  description: string | null;
  mods: string[];
}

export interface ModPack {
  schema_version: number;
  name: string;
  version: string;
  description: string;
  author: string | null;
  collections: Record<string, Collection>;
}

export type ModSource =
  | { type: "mod_io"; mod_id: string }
  | { type: "mod_pack" }
  | { type: "manual" }
  | { type: "collection"; name: string };

export type ModStatus =
  | { type: "not_installed" }
  | { type: "downloading" }
  | { type: "downloaded" }
  | { type: "installed" }
  | { type: "update_available" }
  | { type: "error"; message: string };

export interface ModInfo {
  name: string;
  source: ModSource;
  status: ModStatus;
  filename: string;
  addOns?: InstalledModFile[]; // Add-on files associated with this mod
}

export interface InstalledModFile {
  name: string;
  path: string;
  exists: boolean;
  archiveName?: string;
}

export interface InstalledModGroup {
  name: string;
  displayName?: string;
  sourceUrl?: string;
  managedByManifest: boolean;
  installedAt: number | null;
  files: InstalledModFile[];
  addonFiles?: InstalledModFile[]; // Add-on files for this mod group
  hasOverrideFiles?: boolean;
}

export interface ModProgressEvent {
  operation: string;
  file: string;
  percent: number;
  message: string;
  total_bytes: number | null;
  processed_bytes: number | null;
}

export interface ShareCodeResponse {
  code: string;
  expires_at: string | null;
}

export interface SharedModpack {
  modpack: ModPack;
  shared_by: string;
  created_at: string;
  updated_at: string;
}

export interface Profile {
  name: string;
  description: string | null;
  installed_mod_names: string[];
  enabled_collections: string[];
  collections: Record<string, string[]>;
  tags: Record<string, string[]>;
  collection_colors: Record<string, string>;
  created_at: string;
}

export interface UpdateInfo {
  available: boolean;
  version: string | null;
  notes: string | null;
}

export interface WindowState {
  width: number | null;
  height: number | null;
  x: number | null;
  y: number | null;
}

export type SyncAuth =
  | { type: "Auto" }
  | { type: "Password"; password: string }
  | { type: "KeyFile"; path: string; passphrase: string | null };
