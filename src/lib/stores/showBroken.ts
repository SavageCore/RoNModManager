import { writable } from "svelte/store";

const STORAGE_KEY = "ronmodmanager.showBroken";

function readShowBroken(): boolean {
  if (typeof window === "undefined") return false;
  return localStorage.getItem(STORAGE_KEY) === "true";
}

export const showBroken = writable<boolean>(readShowBroken());

showBroken.subscribe((value) => {
  if (typeof window !== "undefined") {
    localStorage.setItem(STORAGE_KEY, String(value));
  }
});
