import { writable } from "svelte/store";

// Global store for mod enable/disable state
// Maps archive name -> enabled boolean
export const modToggleState = writable<Record<string, boolean>>({});

// Load from localStorage on init
if (typeof window !== "undefined") {
  const saved = localStorage.getItem("modToggleState");
  if (saved) {
    try {
      modToggleState.set(JSON.parse(saved));
    } catch (e) {
      console.warn("Failed to load mod toggle state:", e);
    }
  }
}

// Save to localStorage on every change
modToggleState.subscribe((value) => {
  if (typeof window !== "undefined") {
    localStorage.setItem("modToggleState", JSON.stringify(value));
  }
});

// Helper functions for managing mod state
export const modToggleStateHelpers = {
  /** Remove a single mod from the toggle state */
  remove(modName: string) {
    modToggleState.update((state) => {
      const newState = { ...state };
      delete newState[modName];
      return newState;
    });
  },

  /** Clear all mod toggle state */
  clear() {
    modToggleState.set({});
  },
};
