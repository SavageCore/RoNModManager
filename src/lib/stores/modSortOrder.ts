import { writable } from "svelte/store";

export type SortOrder = "alpha-asc" | "alpha-desc" | "date-asc" | "date-desc";

const STORAGE_KEY = "ronmodmanager.modSortOrder";
const DEFAULT: SortOrder = "alpha-asc";
const VALID: SortOrder[] = ["alpha-asc", "alpha-desc", "date-asc", "date-desc"];

function readSortOrder(): SortOrder {
  if (typeof window === "undefined") return DEFAULT;
  const raw = localStorage.getItem(STORAGE_KEY);
  return VALID.includes(raw as SortOrder) ? (raw as SortOrder) : DEFAULT;
}

export const modSortOrder = writable<SortOrder>(readSortOrder());

modSortOrder.subscribe((value) => {
  if (typeof window !== "undefined") {
    localStorage.setItem(STORAGE_KEY, value);
  }
});
