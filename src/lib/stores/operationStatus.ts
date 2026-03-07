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

const INITIAL_STATE: OperationStatusState = {
  visible: false,
  operation: "",
  file: "",
  percent: 0,
  message: "",
  totalBytes: null,
  processedBytes: null,
  isError: false,
};

let hideTimer: ReturnType<typeof setTimeout> | null = null;

function createOperationStatusStore() {
  const { subscribe, set } = writable<OperationStatusState>(INITIAL_STATE);

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
      });

      if (isComplete || isError) {
        hideTimer = setTimeout(() => {
          set(INITIAL_STATE);
          hideTimer = null;
        }, 5000);
      }
    },
    clear: () => {
      clearHideTimer();
      set(INITIAL_STATE);
    },
  };
}

export const operationStatusStore = createOperationStatusStore();
