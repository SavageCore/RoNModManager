import { writable } from "svelte/store";

export type ToastType = "success" | "error" | "info";

export interface Toast {
  id: string;
  message: string;
  type: ToastType;
  duration: number; // milliseconds, max 30000 (30 seconds)
}

interface ToastStore {
  toasts: Toast[];
}

const MAX_DURATION = 3600000; // 1 hour (effectively allow very long toasts)

function createToastStore() {
  const { subscribe, update } = writable<ToastStore>({ toasts: [] });

  return {
    subscribe,
    add: (
      message: string,
      type: ToastType = "info",
      duration: number = 3000,
    ) => {
      const id = Math.random().toString(36).substring(2, 11);
      const clampedDuration = Math.min(duration, MAX_DURATION);

      update((state) => {
        state.toasts.push({ id, message, type, duration: clampedDuration });
        return state;
      });

      // Auto-remove after duration, unless duration is 0 (persistent)
      if (clampedDuration > 0) {
        setTimeout(() => {
          update((state) => {
            state.toasts = state.toasts.filter((t) => t.id !== id);
            return state;
          });
        }, clampedDuration);
      }

      return id;
    },
    success: (message: string, duration?: number) => {
      return toastStore.add(message, "success", duration);
    },
    error: (message: string, duration?: number) => {
      return toastStore.add(message, "error", duration);
    },
    info: (message: string, duration?: number) => {
      return toastStore.add(message, "info", duration);
    },
    remove: (id: string) => {
      update((state) => {
        state.toasts = state.toasts.filter((t) => t.id !== id);
        return state;
      });
    },
  };
}

export const toastStore = createToastStore();
