import { writable } from "svelte/store";

interface AddModpackPanelState {
  url: string | undefined;
  isOpen: boolean;
  hasActivity: boolean;
  mode: "add" | "update";
  currentVersion: string | null;
  newVersion: string | null;
  doneCounter: number;
  modInstalledCounter: number;
}

function createAddModpackPanelStore() {
  const { subscribe, update } = writable<AddModpackPanelState>({
    isOpen: false,
    hasActivity: false,
    mode: "add",
    currentVersion: null,
    newVersion: null,
    doneCounter: 0,
    modInstalledCounter: 0,
    url: undefined,
  });

  return {
    subscribe,
    open: (
      mode: "add" | "update",
      opts?: {
        currentVersion?: string | null;
        newVersion?: string | null;
        url?: string;
      },
    ) =>
      update((s) => ({
        ...s,
        isOpen: true,
        mode,
        currentVersion: opts?.currentVersion ?? null,
        newVersion: opts?.newVersion ?? null,
        url: opts?.url ?? s.url,
      })),
    close: () => update((s) => ({ ...s, isOpen: false })),
    toggle: () => update((s) => ({ ...s, isOpen: !s.isOpen })),
    setActivity: (hasActivity: boolean) =>
      update((s) => ({ ...s, hasActivity })),
    notifyDone: () => update((s) => ({ ...s, doneCounter: s.doneCounter + 1 })),
    notifyModInstalled: () =>
      update((s) => ({ ...s, modInstalledCounter: s.modInstalledCounter + 1 })),
  };
}

export const addModpackPanelStore = createAddModpackPanelStore();
