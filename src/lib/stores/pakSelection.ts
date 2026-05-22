import { writable } from "svelte/store";
import type { PakFileInfo } from "$lib/api/commands";

interface PakSelectionRequest {
  archiveName: string;
  paks: PakFileInfo[];
  resolve: (selected: string[] | null) => void;
}

export const pakSelectionStore = writable<PakSelectionRequest | null>(null);

export function requestPakSelection(
  archiveName: string,
  paks: PakFileInfo[],
): Promise<string[] | null> {
  return new Promise((resolve) => {
    pakSelectionStore.set({ archiveName, paks, resolve });
  });
}
