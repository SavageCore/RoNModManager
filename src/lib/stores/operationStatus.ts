import { writable } from "svelte/store";
import type { ModProgressEvent } from "$lib/types";

export interface OperationStatusState {
  visible: boolean;
  operation: string;
  file: string;
  percent: number;
  message: string;
  totalBytes: number | null;
  processedBytes: number | null;
  isError: boolean;
}

export interface OperationStatusStateWithTemp extends OperationStatusState {
  /** True if this is a temporary message (e.g. update cooldown), not a real operation. */
  temporary?: boolean;
}

const INITIAL_STATE: OperationStatusStateWithTemp = {
  visible: false,
  operation: "",
  file: "",
  percent: 0,
  message: "",
  totalBytes: null,
  processedBytes: null,
  isError: false,
  temporary: false,
};

let hideTimer: ReturnType<typeof setTimeout> | null = null;

function createOperationStatusStore() {
  const { subscribe, set } =
    writable<OperationStatusStateWithTemp>(INITIAL_STATE);

  function clearHideTimer() {
    if (hideTimer) {
      clearTimeout(hideTimer);
      hideTimer = null;
    }
  }

  return {
    subscribe,
    updateFromProgress: (progress: ModProgressEvent) => {
      clearHideTimer();

      const isComplete = progress.operation === "complete";
      const isError = progress.operation === "error";

      set({
        visible: true,
        operation: progress.operation,
        file: progress.file,
        percent: Math.max(0, Math.min(100, progress.percent ?? 0)),
        message: progress.message,
        totalBytes: progress.total_bytes,
        processedBytes: progress.processed_bytes,
        isError,
        temporary: false,
      });

      if (isComplete || isError) {
        hideTimer = setTimeout(() => {
          set(INITIAL_STATE);
          hideTimer = null;
        }, 5000);
      }
    },
    /** Show a temporary status message in the footer bar for a short time. */
    setTemporaryMessage: (message: string, duration = 3000) => {
      clearHideTimer();
      set({
        ...INITIAL_STATE,
        visible: true,
        message,
        isError: false,
        temporary: true,
      });
      hideTimer = setTimeout(() => {
        set(INITIAL_STATE);
        hideTimer = null;
      }, duration);
    },
    clear: () => {
      clearHideTimer();
      set(INITIAL_STATE);
    },
  };
}

export const operationStatusStore = createOperationStatusStore();
