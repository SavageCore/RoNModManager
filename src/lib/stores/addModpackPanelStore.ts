import { writable } from "svelte/store";

interface AddModpackPanelState {
  isOpen: boolean;
  hasActivity: boolean;
  mode: "add" | "update";
  currentVersion: string | null;
  newVersion: string | null;
  doneCounter: number;
}

function createAddModpackPanelStore() {
  const { subscribe, update } = writable<AddModpackPanelState>({
    isOpen: false,
    hasActivity: false,
    mode: "add",
    currentVersion: null,
    newVersion: null,
    doneCounter: 0,
  });

  return {
    subscribe,
    open: (
      mode: "add" | "update",
      opts?: { currentVersion?: string | null; newVersion?: string | null },
    ) =>
      update((s) => ({
        ...s,
        isOpen: true,
        mode,
        currentVersion: opts?.currentVersion ?? null,
        newVersion: opts?.newVersion ?? null,
      })),
    close: () => update((s) => ({ ...s, isOpen: false })),
    toggle: () => update((s) => ({ ...s, isOpen: !s.isOpen })),
    setActivity: (hasActivity: boolean) =>
      update((s) => ({ ...s, hasActivity })),
    notifyDone: () => update((s) => ({ ...s, doneCounter: s.doneCounter + 1 })),
  };
}

export const addModpackPanelStore = createAddModpackPanelStore();
