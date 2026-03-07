import { writable } from "svelte/store";

export type QueueStatus = "queued" | "running" | "done" | "error";

export interface ModAddQueueItem {
  id: string;
  input: string;
  status: QueueStatus;
  message: string;
  createdAt: number;
}

export interface ModAddQueueState {
  items: ModAddQueueItem[];
  totalQueued: number; // Track total items queued in current batch
}

const MAX_HISTORY = 20;

function createModAddQueueStore() {
  const { subscribe, update } = writable<ModAddQueueState>({
    items: [],
    totalQueued: 0,
  });

  function trimHistory(items: ModAddQueueItem[]): ModAddQueueItem[] {
    const active = items.filter(
      (item) => item.status === "queued" || item.status === "running",
    );
    const history = items
      .filter((item) => item.status === "done" || item.status === "error")
      .sort((a, b) => b.createdAt - a.createdAt)
      .slice(0, MAX_HISTORY);
    return [...active, ...history].sort((a, b) => a.createdAt - b.createdAt);
  }

  return {
    subscribe,
    enqueue: (input: string) => {
      const id = Math.random().toString(36).slice(2, 11);
      const now = Date.now();
      update((state) => ({
        items: [
          ...state.items,
          {
            id,
            input,
            status: "queued",
            message: "Queued",
            createdAt: now,
          },
        ],
        totalQueued: state.totalQueued + 1,
      }));
      return id;
    },
    markRunning: (id: string, message = "Downloading") => {
      update((state) => ({
        items: state.items.map((item) =>
          item.id === id ? { ...item, status: "running", message } : item,
        ),
        totalQueued: state.totalQueued,
      }));
    },
    markDone: (id: string, message: string) => {
      update((state) => ({
        items: trimHistory(
          state.items.map((item) =>
            item.id === id ? { ...item, status: "done", message } : item,
          ),
        ),
        totalQueued: state.totalQueued,
      }));
    },
    markError: (id: string, message: string) => {
      update((state) => ({
        items: trimHistory(
          state.items.map((item) =>
            item.id === id ? { ...item, status: "error", message } : item,
          ),
        ),
        totalQueued: state.totalQueued,
      }));
    },
    clearHistory: () => {
      update((state) => ({
        items: state.items.filter(
          (item) => item.status === "queued" || item.status === "running",
        ),
        totalQueued: state.totalQueued,
      }));
    },
    resetBatch: () => {
      update((state) => ({
        items: state.items.filter(
          (item) => item.status === "queued" || item.status === "running",
        ),
        totalQueued: 0,
      }));
    },
  };
}

export const modAddQueueStore = createModAddQueueStore();
