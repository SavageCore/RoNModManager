import { writable } from "svelte/store";
import type { ModUpdateInfo } from "$lib/api/commands";

/** Re-check cadence for automatic mod-update checks (1 hour). */
export const MOD_UPDATES_AUTO_CHECK_INTERVAL_MS = 60 * 60 * 1000;

const LAST_CHECKED_KEY = "ronmodmanager.modUpdatesLastCheckedAt";
const UPDATES_KEY = "ronmodmanager.modUpdates";

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

function readCachedUpdates(): Record<string, ModUpdateInfo> {
  if (typeof window === "undefined") return {};
  try {
    const raw = window.localStorage.getItem(UPDATES_KEY);
    if (!raw) return {};
    const parsed: unknown = JSON.parse(raw);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      return parsed as Record<string, ModUpdateInfo>;
    }
  } catch {
    // Corrupt cache: treat as empty; next check repopulates.
  }
  return {};
}

function persistUpdates(updates: Record<string, ModUpdateInfo>): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(UPDATES_KEY, JSON.stringify(updates));
  } catch {
    // Storage full or unavailable: throttle timestamp still persists.
  }
}

function clearCachedUpdates(): void {
  if (typeof window === "undefined") return;
  window.localStorage.removeItem(UPDATES_KEY);
}

export interface ModUpdatesState {
  updates: Record<string, ModUpdateInfo>;
  lastCheckedAt: number | null;
}

/**
 * One-line footer text for the cached result, shown when a launch skips the
 * check because a fresh result already exists. Null when there is no cached
 * check at all (leave the footer alone).
 */
export function describeModUpdatesStatus(
  updates: Record<string, ModUpdateInfo> | null,
): string | null {
  if (!updates) return null;
  const count = Object.keys(updates).length;
  if (count === 0) return "Mods up to date";
  return `${count} mod update${count === 1 ? "" : "s"} available — see Mods page`;
}

function createModUpdatesStore() {
  const { subscribe, set, update } = writable<ModUpdatesState>({
    updates: readCachedUpdates(),
    lastCheckedAt: readLastCheckedAt(),
  });

  return {
    subscribe,
    setUpdates: (updates: Record<string, ModUpdateInfo>) => {
      const now = Date.now();
      persistLastCheckedAt(now);
      persistUpdates(updates);
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
      clearCachedUpdates();
      set({ updates: {}, lastCheckedAt: null });
    },
  };
}

export const modUpdatesStore = createModUpdatesStore();
