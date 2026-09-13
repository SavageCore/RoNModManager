import { writable } from "svelte/store";

export interface PendingInstall {
  /**
   * The Nexus/Mod.io URL (or mod.io id string) that AddModModal submits.
   */
  url: string;
  /**
   * Pre-selected Nexus file IDs to download (multi-part / variant picker was
   * resolved in the userscript). When empty, the app falls back to its own
   * file-variant picker.
   */
  fileIds?: number[];
  /**
   * Whether the browser tab open for a free Nexus download was already started
   * in-page by the userscript. When true the backend skips its own tab open.
   */
  skipBrowserOpen?: boolean;
  /**
   * When true (and >1 file selected), extra files should be linked as add-ons
   * of the first installed file instead of becoming separate top-level mods.
   */
  linkAsAddons?: boolean;
}

export const pendingInstallUrl = writable<string | null>(null);
export const pendingInstall = writable<PendingInstall | null>(null);
