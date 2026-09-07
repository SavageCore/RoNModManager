import { writable } from "svelte/store";
import { modAddQueueStore, type QueueStatus } from "./modAddQueue";
import { operationStatusStore } from "./operationStatus";

export interface ImportLogMod {
  id: string;
  input: string;
  lines: string[];
  status: "queued" | "running" | "done" | "error";
  isActive: boolean;
  expanded: boolean;
  awaitingInput: boolean;
}

// Backend log level mirrored for the Import Log. When "debug" or "trace",
// phase transitions include archive names + byte totals so the saved log
// captures what the footer flickered past. Set via setLogLevel(), typically
// from the Settings page after hydrating the config.
let currentLogLevel = "info";

const VERBOSE_LEVELS = new Set(["debug", "trace"]);

export function setLogLevel(level: string): void {
  currentLogLevel = level;
}

function formatBytes(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "0 B";
  const units = ["B", "KiB", "MiB", "GiB"];
  let size = value;
  let i = 0;
  while (size >= 1024 && i < units.length - 1) {
    size /= 1024;
    i += 1;
  }
  return `${size.toFixed(size >= 10 || i === 0 ? 0 : 1)} ${units[i]}`;
}

interface ImportLogState {
  mods: ImportLogMod[];
  isOpen: boolean;
}

function phaseLabel(operation: string, message: string): string {
  if (operation.includes("download")) {
    if (
      message.startsWith("Waiting for") ||
      message.startsWith("Opening Nexus")
    ) {
      return "Waiting for download...";
    }
    return "Downloading...";
  }
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
  const seenLabels = new Set<string>();
  const prevStatuses = new Map<string, QueueStatus>();

  modAddQueueStore.subscribe((state) => {
    for (const item of state.items) {
      const prev = prevStatuses.get(item.id);
      if (prev !== item.status) {
        if (item.status === "queued") {
          update((s) => {
            if (s.mods.some((m) => m.id === item.id)) return s;
            return {
              ...s,
              mods: [
                ...s.mods,
                {
                  id: item.id,
                  input: item.input,
                  lines: [],
                  status: "queued",
                  isActive: false,
                  expanded: true,
                  awaitingInput: false,
                },
              ],
            };
          });
        } else if (item.status === "running") {
          currentModId = item.id;
          seenLabels.clear();
          update((s) => ({
            ...s,
            isOpen: true,
            mods: [
              ...s.mods
                .filter((m) => m.id !== item.id)
                .map((m) => ({ ...m, isActive: false })),
              {
                id: item.id,
                input: item.input,
                lines: [],
                status: "running",
                isActive: true,
                expanded: true,
                awaitingInput: false,
              },
            ],
          }));
        } else if (item.status === "done" || item.status === "error") {
          if (currentModId === item.id) {
            currentModId = null;
            seenLabels.clear();
          }
          update((s) => ({
            ...s,
            mods: s.mods.map((m) =>
              m.id === item.id
                ? {
                    ...m,
                    status: item.status as "done" | "error",
                    isActive: false,
                    awaitingInput: false,
                    lines:
                      item.status === "done"
                        ? [item.message]
                        : [...m.lines, item.message],
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
      if (!state.visible) seenLabels.clear();
      return;
    }
    if (state.operation === "complete" || state.operation === "error") return;
    // Downloads run concurrently with installs, so their progress events
    // can't be attributed to a single mod - the queue status text already
    // shows waiting/downloading state per row.
    if (state.operation.includes("download")) return;
    const label = phaseLabel(state.operation, state.message);
    if (!seenLabels.has(label)) {
      seenLabels.add(label);
      update((s) => ({
        ...s,
        mods: s.mods.map((m) =>
          m.id === currentModId ? { ...m, lines: [...m.lines, label] } : m,
        ),
      }));
    }
    // Verbose: append a timestamped detail line with the archive/file name
    // and byte totals so the saved log captures what the footer flickered
    // past. Only at debug/trace to keep the default log concise.
    if (VERBOSE_LEVELS.has(currentLogLevel) && state.operation) {
      const detail = `[${new Date().toISOString().slice(11, 19)}] ${
        state.file ? state.file + " " : ""
      }${state.message}${
        state.totalBytes != null && state.processedBytes != null
          ? ` (${formatBytes(state.processedBytes)}/${formatBytes(state.totalBytes)})`
          : ""
      }`;
      update((s) => ({
        ...s,
        mods: s.mods.map((m) =>
          m.id === currentModId ? { ...m, lines: [...m.lines, detail] } : m,
        ),
      }));
    }
  });

  return {
    subscribe,
    toggle: () => update((s) => ({ ...s, isOpen: !s.isOpen })),
    open: () => update((s) => ({ ...s, isOpen: true })),
    close: () => update((s) => ({ ...s, isOpen: false })),
    clear: () => {
      currentModId = null;
      seenLabels.clear();
      update((s) => ({ ...s, mods: [] }));
    },
    setCurrentMod: (id: string) => {
      currentModId = id;
      seenLabels.clear();
      update((s) => ({
        ...s,
        mods: s.mods.map((m) =>
          m.id === id
            ? { ...m, isActive: true, expanded: true }
            : { ...m, isActive: false },
        ),
      }));
    },
    toggleExpanded: (id: string) => {
      update((s) => ({
        ...s,
        mods: s.mods.map((m) =>
          m.id === id ? { ...m, expanded: !m.expanded } : m,
        ),
      }));
    },
    setWaitingForInput: (id: string) => {
      update((s) => ({
        ...s,
        mods: s.mods.map((m) =>
          m.id === id ? { ...m, awaitingInput: true } : m,
        ),
      }));
    },
    clearWaitingForInput: (id: string) => {
      update((s) => ({
        ...s,
        mods: s.mods.map((m) =>
          m.id === id ? { ...m, awaitingInput: false } : m,
        ),
      }));
    },
  };
}

export const importLogStore = createImportLogStore();
