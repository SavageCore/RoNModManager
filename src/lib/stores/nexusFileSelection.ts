import { writable } from "svelte/store";
import type { NexusFileOption } from "$lib/api/commands";

interface NexusFileSelectionRequest {
  modName: string;
  files: NexusFileOption[];
  resolve: (selected: NexusFileOption | null) => void;
}

export const nexusFileSelectionStore =
  writable<NexusFileSelectionRequest | null>(null);

export function requestNexusFileSelection(
  modName: string,
  files: NexusFileOption[],
): Promise<NexusFileOption | null> {
  return new Promise((resolve) => {
    nexusFileSelectionStore.set({ modName, files, resolve });
  });
}
