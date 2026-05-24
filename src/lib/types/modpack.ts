// Shared type for modpack mod entries
export type ModInfo = {
  source_url?: string;
  content_hash?: string;
  selected_pak_files?: string[];
  nexus_file_id?: number;
  [key: string]: any;
};
