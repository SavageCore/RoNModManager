import { writable } from "svelte/store";

export interface ManualDownloadFile {
  prettyName: string | null;
  fileName: string;
  modUrl: string;
  /** Per-wait backend id for targeted cancellation. Null for entries added
   * before wait ids existed (defensive - current backend always sends one). */
  waitId: number | null;
}

export interface ManualDownloadState {
  pendingFiles: ManualDownloadFile[];
  dismissed: boolean;
}

function createManualDownloadStore() {
  const { subscribe, update } = writable<ManualDownloadState>({
    pendingFiles: [],
    dismissed: false,
  });

  return {
    subscribe,
    add: (file: ManualDownloadFile) =>
      update((s) => {
        // Dedupe by file name only. Sibling files from one mod share a
        // modUrl, and keying on it would collapse them into a single row so
        // the user never sees the rest.
        const existing = s.pendingFiles.find(
          (d) => d.fileName === file.fileName,
        );
        if (existing) {
          return {
            ...s,
            pendingFiles: s.pendingFiles.map((d) =>
              d === existing ? { ...file, queued: false } : d,
            ),
            dismissed: false,
          };
        }
        return {
          ...s,
          pendingFiles: [...s.pendingFiles, file],
          dismissed: false,
        };
      }),
    remove: (fileName: string) =>
      update((s) => {
        const pendingFiles = s.pendingFiles.filter(
          (d) => d.fileName !== fileName,
        );
        return {
          ...s,
          pendingFiles,
          dismissed: pendingFiles.length === 0 ? false : s.dismissed,
        };
      }),
    dismiss: () => update((s) => ({ ...s, dismissed: true })),
    reopen: () => update((s) => ({ ...s, dismissed: false })),
    clear: () => update(() => ({ pendingFiles: [], dismissed: false })),
  };
}

export const manualDownloadStore = createManualDownloadStore();
