import { writable } from "svelte/store";

const STORAGE_KEY = "ronmodmanager.ue4ssBannerDismissed";

function readDismissed(): boolean {
  if (typeof window === "undefined") return false;
  return localStorage.getItem(STORAGE_KEY) === "true";
}

export const ue4ssBannerDismissed = writable<boolean>(readDismissed());

ue4ssBannerDismissed.subscribe((value) => {
  if (typeof window !== "undefined") {
    localStorage.setItem(STORAGE_KEY, String(value));
  }
});
