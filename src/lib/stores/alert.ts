import { writable } from "svelte/store";

export type AlertType = "success" | "error" | "info";

export interface AlertState {
  message: string;
  type: AlertType;
}

const INITIAL_STATE: AlertState = {
  message: "",
  type: "info",
};

function createAlertStore() {
  const { subscribe, set } = writable<AlertState>(INITIAL_STATE);

  return {
    subscribe,
    set: (message: string, type: AlertType = "info") => {
      set({ message, type });
    },
    success: (message: string) => {
      set({ message, type: "success" });
    },
    error: (message: string) => {
      set({ message, type: "error" });
    },
    info: (message: string) => {
      set({ message, type: "info" });
    },
    clear: () => {
      set(INITIAL_STATE);
    },
  };
}

export const alertStore = createAlertStore();
