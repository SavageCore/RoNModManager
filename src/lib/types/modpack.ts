// Shared type for modpack mod entries
export type ModInfo = {
  source_url?: string;
  content_hash?: string;
  selected_pak_files?: string[];
  [key: string]: any;
};
