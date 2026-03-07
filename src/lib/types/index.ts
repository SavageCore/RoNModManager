export type ThemeMode = "light" | "dark" | "system";

export interface SubscribedMod {
  md5: string;
  filename: string;
  download_url: string;
  contents: string[];
}

export interface AppConfig {
  game_path: string | null;
  modpack_url: string | null;
  modpack_version: string | null;
  oauth_token: string | null;
  subscribed_mods: Record<string, SubscribedMod>;
  collections: Record<string, boolean>;
  theme: ThemeMode;
  last_update_check: string | null;
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
  subscriptions: string[];
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
}

export interface ProgressEvent {
  task: string;
  current: number;
  total: number;
  message: string;
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
