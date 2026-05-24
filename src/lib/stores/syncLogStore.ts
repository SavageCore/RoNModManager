import { writable } from "svelte/store";

interface SyncLogState {
  log: string[];
  isOpen: boolean;
  isBusy: boolean;
}

function createSyncLogStore() {
  const { subscribe, update } = writable<SyncLogState>({
    log: [],
    isOpen: false,
    isBusy: false,
  });

  return {
    subscribe,
    start: () => update(() => ({ log: [], isOpen: true, isBusy: true })),
    addLine: (line: string) => update((s) => ({ ...s, log: [...s.log, line] })),
    finish: () => update((s) => ({ ...s, isBusy: false })),
    toggle: () => update((s) => ({ ...s, isOpen: !s.isOpen })),
    close: () => update((s) => ({ ...s, isOpen: false })),
    clear: () => update((s) => ({ ...s, log: [] })),
  };
}

export const syncLogStore = createSyncLogStore();
