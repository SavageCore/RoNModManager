import { writable } from "svelte/store";

interface InfoLogState {
  lines: string[];
  isOpen: boolean;
  isBusy: boolean;
  tone: "idle" | "success" | "error";
}

function createInfoLogStore() {
  const { subscribe, update } = writable<InfoLogState>({
    lines: [],
    isOpen: false,
    isBusy: false,
    tone: "idle",
  });

  return {
    subscribe,
    start: () =>
      update(() => ({ lines: [], isOpen: true, isBusy: true, tone: "idle" })),
    addLine: (line: string) =>
      update((s) => ({ ...s, lines: [...s.lines, line] })),
    finish: (tone: "idle" | "success" | "error") =>
      update((s) => ({ ...s, isBusy: false, tone })),
    toggle: () => update((s) => ({ ...s, isOpen: !s.isOpen })),
    close: () => update((s) => ({ ...s, isOpen: false })),
    clear: () =>
      update((s) => ({ ...s, lines: [], tone: "idle", isOpen: false })),
  };
}

export const infoLogStore = createInfoLogStore();
