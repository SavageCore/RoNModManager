import { writable } from "svelte/store";

export type ToastType = "success" | "error" | "info" | "warning";

export interface ToastAction {
  label: string;
  handler: () => void | Promise<void>;
}

export interface Toast {
  id: string;
  message: string;
  type: ToastType;
  duration: number; // milliseconds, max 30000 (30 seconds)
  action?: ToastAction;
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
      duration?: number,
      action?: ToastAction,
    ) => {
      const defaults: Record<ToastType, number> = {
        success: 4000,
        info: 4000,
        warning: 8000,
        error: 8000,
      };
      const id = Math.random().toString(36).substring(2, 11);
      const clampedDuration = Math.min(
        duration ?? defaults[type],
        MAX_DURATION,
      );

      update((state) => {
        state.toasts.push({
          id,
          message,
          type,
          duration: clampedDuration,
          action,
        });
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
    success: (message: string, duration?: number, action?: ToastAction) => {
      return toastStore.add(message, "success", duration, action);
    },
    error: (message: string, duration?: number, action?: ToastAction) => {
      return toastStore.add(message, "error", duration, action);
    },
    info: (message: string, duration?: number, action?: ToastAction) => {
      return toastStore.add(message, "info", duration, action);
    },
    warning: (message: string, duration?: number, action?: ToastAction) => {
      return toastStore.add(message, "warning", duration, action);
    },
    fromError: (err: unknown, duration?: number) => {
      const message = err instanceof Error ? err.message : String(err);
      return toastStore.add(message, "error", duration);
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
