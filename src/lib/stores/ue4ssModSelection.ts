import { writable } from "svelte/store";
import type { Ue4ssModFolder } from "$lib/api/commands";

interface Ue4ssModSelectionRequest {
  archiveName: string;
  mods: Ue4ssModFolder[];
  resolve: (selected: string[] | null) => void;
}

export const ue4ssModSelectionStore = writable<Ue4ssModSelectionRequest | null>(
  null,
);

export function requestUe4ssModSelection(
  archiveName: string,
  mods: Ue4ssModFolder[],
): Promise<string[] | null> {
  return new Promise((resolve) => {
    ue4ssModSelectionStore.set({ archiveName, mods, resolve });
  });
}
