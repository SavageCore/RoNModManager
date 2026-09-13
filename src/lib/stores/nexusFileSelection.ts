import { writable } from "svelte/store";
import type { NexusFileOption } from "$lib/api/commands";

interface NexusFileSelectionRequest {
  modName: string;
  files: NexusFileOption[];
  resolve: (selected: NexusFileSelectionResult | null) => void;
}

/**
 * Multi-part selection result. `linkAsAddons` is only true when more than one
 * file is selected: every file after the first should be registered as an
 * add-on of the first installed file instead of a separate top-level mod.
 */
export interface NexusFileSelectionResult {
  files: NexusFileOption[];
  linkAsAddons: boolean;
}

export const nexusFileSelectionStore =
  writable<NexusFileSelectionRequest | null>(null);

export function requestNexusFileSelection(
  modName: string,
  files: NexusFileOption[],
): Promise<NexusFileSelectionResult | null> {
  return new Promise((resolve) => {
    nexusFileSelectionStore.set({ modName, files, resolve });
  });
}
