import { writable } from "svelte/store";
import { modAddQueueStore, type QueueStatus } from "./modAddQueue";
import { operationStatusStore } from "./operationStatus";

export interface ImportLogMod {
  id: string;
  input: string;
  lines: string[];
  status: "running" | "done" | "error";
}

interface ImportLogState {
  mods: ImportLogMod[];
  isOpen: boolean;
}

function phaseLabel(operation: string): string {
  if (operation.includes("download")) return "Downloading...";
  if (operation === "install") return "Preparing...";
  if (operation === "hash") return "Verifying archive...";
  if (operation === "dedupe") return "Checking for duplicates...";
  if (operation === "extract") return "Extracting files...";
  return operation;
}

function createImportLogStore() {
  const { subscribe, update } = writable<ImportLogState>({
    mods: [],
    isOpen: false,
  });

  let currentModId: string | null = null;
  let lastOperation = "";
  const prevStatuses = new Map<string, QueueStatus>();

  modAddQueueStore.subscribe((state) => {
    for (const item of state.items) {
      const prev = prevStatuses.get(item.id);
      if (prev !== item.status) {
        if (item.status === "running") {
          currentModId = item.id;
          lastOperation = "";
          update((s) => ({
            ...s,
            isOpen: true,
            mods: [
              ...s.mods,
              { id: item.id, input: item.input, lines: [], status: "running" },
            ],
          }));
        } else if (item.status === "done" || item.status === "error") {
          if (currentModId === item.id) {
            currentModId = null;
            lastOperation = "";
          }
          update((s) => ({
            ...s,
            mods: s.mods.map((m) =>
              m.id === item.id
                ? {
                    ...m,
                    status: item.status as "done" | "error",
                    lines: [...m.lines, item.message],
                  }
                : m,
            ),
          }));
        }
      }
      prevStatuses.set(item.id, item.status);
    }
    // Clean up items no longer tracked by the queue
    const ids = new Set(state.items.map((i) => i.id));
    for (const id of [...prevStatuses.keys()]) {
      if (!ids.has(id)) prevStatuses.delete(id);
    }
  });

  operationStatusStore.subscribe((state) => {
    if (!state.visible || !state.operation || !currentModId) {
      if (!state.visible) lastOperation = "";
      return;
    }
    if (
      state.operation !== lastOperation &&
      state.operation !== "complete" &&
      state.operation !== "error"
    ) {
      lastOperation = state.operation;
      const label = phaseLabel(state.operation);
      update((s) => ({
        ...s,
        mods: s.mods.map((m) =>
          m.id === currentModId ? { ...m, lines: [...m.lines, label] } : m,
        ),
      }));
    }
  });

  return {
    subscribe,
    toggle: () => update((s) => ({ ...s, isOpen: !s.isOpen })),
    open: () => update((s) => ({ ...s, isOpen: true })),
    close: () => update((s) => ({ ...s, isOpen: false })),
    clear: () => update((s) => ({ ...s, mods: [] })),
  };
}

export const importLogStore = createImportLogStore();
