import { writable } from "svelte/store";

const UPDATE_LAST_CHECKED_KEY = "ronmodmanager.updateLastCheckedAt";

function readLastCheckedAt(): number | null {
  if (typeof window === "undefined") {
    return null;
  }

  const raw = window.localStorage.getItem(UPDATE_LAST_CHECKED_KEY);
  if (!raw) {
    return null;
  }

  const parsed = Number(raw);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return null;
  }

  return parsed;
}

function persistLastCheckedAt(value: number | null): void {
  if (typeof window === "undefined") {
    return;
  }

  if (value === null) {
    window.localStorage.removeItem(UPDATE_LAST_CHECKED_KEY);
    return;
  }

  window.localStorage.setItem(UPDATE_LAST_CHECKED_KEY, String(value));
}

function createUpdateCheckStore() {
  const { subscribe, set } = writable<number | null>(readLastCheckedAt());

  return {
    subscribe,
    markChecked: () => {
      const now = Date.now();
      set(now);
      persistLastCheckedAt(now);
      return now;
    },
    setLastCheckedAt: (value: number | null) => {
      set(value);
      persistLastCheckedAt(value);
    },
    clear: () => {
      set(null);
      persistLastCheckedAt(null);
    },
  };
}

export const updateCheckStore = createUpdateCheckStore();
