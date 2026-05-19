// Shared type for modpack mod entries
export type ModInfo = {
  source_url?: string;
  content_hash?: string;
  [key: string]: any;
};
