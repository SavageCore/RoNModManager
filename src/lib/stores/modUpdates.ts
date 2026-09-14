import { writable } from "svelte/store";
import type { ModUpdateInfo } from "$lib/api/commands";

/** Re-check cadence for automatic mod-update checks (1 hour). */
export const MOD_UPDATES_AUTO_CHECK_INTERVAL_MS = 60 * 60 * 1000;

const LAST_CHECKED_KEY = "ronmodmanager.modUpdatesLastCheckedAt";

function readLastCheckedAt(): number | null {
  if (typeof window === "undefined") return null;
  const raw = window.localStorage.getItem(LAST_CHECKED_KEY);
  if (!raw) return null;
  const parsed = Number(raw);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : null;
}

function persistLastCheckedAt(value: number | null): void {
  if (typeof window === "undefined") return;
  if (value === null) window.localStorage.removeItem(LAST_CHECKED_KEY);
  else window.localStorage.setItem(LAST_CHECKED_KEY, String(value));
}

export interface ModUpdatesState {
  updates: Record<string, ModUpdateInfo>;
  lastCheckedAt: number | null;
}

function createModUpdatesStore() {
  const { subscribe, set, update } = writable<ModUpdatesState>({
    updates: {},
    lastCheckedAt: readLastCheckedAt(),
  });

  return {
    subscribe,
    setUpdates: (updates: Record<string, ModUpdateInfo>) => {
      const now = Date.now();
      persistLastCheckedAt(now);
      set({ updates, lastCheckedAt: now });
    },
    setLastCheckedAt: (value: number | null) => {
      persistLastCheckedAt(value);
      update((s) => ({ ...s, lastCheckedAt: value }));
    },
    isStale: () => {
      let current: ModUpdatesState = { updates: {}, lastCheckedAt: null };
      const unsub = subscribe((s) => (current = s));
      unsub();
      if (!current.lastCheckedAt) return true;
      return (
        Date.now() - current.lastCheckedAt >= MOD_UPDATES_AUTO_CHECK_INTERVAL_MS
      );
    },
    clear: () => {
      persistLastCheckedAt(null);
      set({ updates: {}, lastCheckedAt: null });
    },
  };
}

export const modUpdatesStore = createModUpdatesStore();
