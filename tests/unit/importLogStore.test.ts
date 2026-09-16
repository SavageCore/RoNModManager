import { beforeEach, describe, expect, it } from "vitest";
import { get } from "svelte/store";
import {
  importLogStore,
  type ImportLogMod,
} from "../../src/lib/stores/importLogStore";
import { modAddQueueStore } from "../../src/lib/stores/modAddQueue";
import { operationStatusStore } from "../../src/lib/stores/operationStatus";

/** Resolve a mod progress event with sensible defaults. */
function progress(operation: string, message = "") {
  operationStatusStore.updateFromProgress({
    operation,
    file: "mod.zip",
    percent: 50,
    message,
    total_bytes: null,
    processed_bytes: null,
  });
}

function entry(id: string): ImportLogMod | undefined {
  return get(importLogStore).mods.find((m) => m.id === id);
}

/** Drop every queued item so each test starts from an empty log. */
function drainQueue() {
  for (const item of get(modAddQueueStore).items) {
    modAddQueueStore.markDone(item.id, "drained");
  }
  modAddQueueStore.clearHistory();
  modAddQueueStore.resetBatch();
  importLogStore.clear();
  importLogStore.close();
  operationStatusStore.clear();
}

beforeEach(() => {
  drainQueue();
});

describe("importLogStore", () => {
  it("logs a mod from queued through running to done", () => {
    const id = modAddQueueStore.enqueue(
      "https://www.nexusmods.com/readyornot/mods/1234",
      "Alpha",
    );
    expect(entry(id)).toMatchObject({
      input: "https://www.nexusmods.com/readyornot/mods/1234",
      name: "Alpha",
      status: "queued",
      isActive: false,
      expanded: true,
      awaitingInput: false,
      lines: [],
    });
    // Queuing alone does not pop the panel open.
    expect(get(importLogStore).isOpen).toBe(false);

    modAddQueueStore.markRunning(id);
    expect(entry(id)).toMatchObject({ status: "running", isActive: true });
    expect(get(importLogStore).isOpen).toBe(true);
  });

  it("appends each phase line once and keeps them on completion", () => {
    const id = modAddQueueStore.enqueue("input", "Alpha");
    modAddQueueStore.markRunning(id);

    progress("hash", "Verifying archive");
    // Per-tick repeats of the same phase are folded into one line.
    progress("hash", "Verifying archive (2/4)");
    progress("extract", "Extracting files");
    expect(entry(id)?.lines).toEqual([
      "Verifying archive...",
      "Extracting files...",
    ]);

    modAddQueueStore.markDone(id, "Installed");
    expect(entry(id)).toMatchObject({
      status: "done",
      isActive: false,
      expanded: false,
      awaitingInput: false,
    });
    // The completion message is appended, not swapped in for the phases.
    expect(entry(id)?.lines).toEqual([
      "Verifying archive...",
      "Extracting files...",
      "Installed",
    ]);
  });

  it("marks errors and keeps the phase history", () => {
    const id = modAddQueueStore.enqueue("bad-input", "Broken");
    modAddQueueStore.markRunning(id, "Downloading");
    progress("install");
    modAddQueueStore.markError(id, "Download failed");

    expect(entry(id)).toMatchObject({ status: "error", isActive: false });
    expect(entry(id)?.lines).toEqual(["Preparing...", "Download failed"]);
  });

  it("deactivates the previous row when the next mod starts", () => {
    const first = modAddQueueStore.enqueue("first", "First");
    const second = modAddQueueStore.enqueue("second", "Second");

    modAddQueueStore.markRunning(first);
    expect(entry(first)?.isActive).toBe(true);
    expect(entry(second)?.isActive).toBe(false);
    expect(entry(second)?.status).toBe("queued");

    modAddQueueStore.markRunning(second);
    expect(entry(first)?.isActive).toBe(false);
    expect(entry(second)?.isActive).toBe(true);
  });

  it("picks up names resolved after the mod was queued", () => {
    const id = modAddQueueStore.enqueue("https://mod.io/g/readyornot/m/alpha");
    expect(entry(id)?.name).toBeUndefined();
    // A second, already named row keeps its own name.
    const other = modAddQueueStore.enqueue("second", "Second");

    modAddQueueStore.setName(id, "Alpha");
    expect(entry(id)?.name).toBe("Alpha");
    expect(entry(id)?.input).toBe("https://mod.io/g/readyornot/m/alpha");
    expect(entry(other)?.name).toBe("Second");
  });

  it("records a queued mod failing while another one runs", () => {
    const runner = modAddQueueStore.enqueue("runner", "Runner");
    const waiting = modAddQueueStore.enqueue("waiting", "Waiting");
    modAddQueueStore.markRunning(runner);

    modAddQueueStore.markError(waiting, "Could not parse link");

    expect(entry(waiting)).toMatchObject({ status: "error", isActive: false });
    expect(entry(waiting)?.lines).toEqual(["Could not parse link"]);
    // The running mod still owns the phase lines.
    progress("install");
    expect(entry(runner)?.lines).toEqual(["Preparing..."]);
    expect(entry(waiting)?.lines).toEqual(["Could not parse link"]);
  });

  it("ignores phase events while the footer is hidden", () => {
    const id = modAddQueueStore.enqueue("input", "Alpha");
    modAddQueueStore.markRunning(id);
    progress("install");
    expect(entry(id)?.lines).toEqual(["Preparing..."]);

    // Hiding the footer clears the seen labels without touching the log.
    operationStatusStore.clear();
    expect(entry(id)?.lines).toEqual(["Preparing..."]);
  });

  it("ignores phase events when no mod is running", () => {
    const id = modAddQueueStore.enqueue("input", "Alpha");
    modAddQueueStore.markRunning(id);
    modAddQueueStore.markDone(id, "Installed");

    progress("install", "Preparing");
    progress("extract", "Extracting");
    expect(entry(id)?.lines).toEqual(["Installed"]);
  });

  it("skips download events and terminal states", () => {
    const id = modAddQueueStore.enqueue("input", "Alpha");
    modAddQueueStore.markRunning(id);

    // Downloads run alongside installs, so they cannot be attributed.
    progress("download", "Downloading 42%");
    progress("complete", "Done");
    progress("error", "Failed");
    expect(entry(id)?.lines).toEqual([]);

    progress("dedupe");
    expect(entry(id)?.lines).toEqual(["Checking for duplicates..."]);
  });

  it("applies phase labels to an explicitly selected mod", () => {
    const first = modAddQueueStore.enqueue("first", "First");
    const second = modAddQueueStore.enqueue("second", "Second");
    modAddQueueStore.markRunning(second);
    modAddQueueStore.markDone(second, "Installed");

    importLogStore.setCurrentMod(first);
    progress("install");
    expect(entry(first)?.lines).toEqual(["Preparing..."]);
    expect(entry(second)?.lines).toEqual(["Installed"]);
  });

  it("opens, closes, toggles, expands and clears the log", () => {
    const id = modAddQueueStore.enqueue("input", "Alpha");
    expect(get(importLogStore).isOpen).toBe(false);

    importLogStore.open();
    expect(get(importLogStore).isOpen).toBe(true);
    importLogStore.toggle();
    expect(get(importLogStore).isOpen).toBe(false);
    importLogStore.close();
    expect(get(importLogStore).isOpen).toBe(false);

    importLogStore.toggleExpanded(id);
    expect(entry(id)?.expanded).toBe(false);
    importLogStore.toggleExpanded(id);
    expect(entry(id)?.expanded).toBe(true);

    importLogStore.setWaitingForInput(id);
    expect(entry(id)?.awaitingInput).toBe(true);
    importLogStore.clearWaitingForInput(id);
    expect(entry(id)?.awaitingInput).toBe(false);

    importLogStore.clear();
    expect(get(importLogStore).mods).toEqual([]);
  });

  it("ignores actions aimed at an unknown mod id", () => {
    const id = modAddQueueStore.enqueue("input", "Alpha");
    importLogStore.setWaitingForInput(id);

    importLogStore.setCurrentMod("missing-id");
    importLogStore.toggleExpanded("missing-id");
    importLogStore.setWaitingForInput("missing-id");
    importLogStore.clearWaitingForInput("missing-id");

    expect(entry(id)).toMatchObject({
      expanded: true,
      awaitingInput: true,
      isActive: false,
    });

    importLogStore.clearWaitingForInput(id);
    expect(entry(id)?.awaitingInput).toBe(false);
  });

  it("activates one row at a time via setCurrentMod", () => {
    const first = modAddQueueStore.enqueue("first", "First");
    const second = modAddQueueStore.enqueue("second", "Second");
    modAddQueueStore.markRunning(second);
    modAddQueueStore.markDone(second, "Installed");

    importLogStore.setCurrentMod(first);
    expect(entry(first)).toMatchObject({ isActive: true, expanded: true });
    expect(entry(second)?.isActive).toBe(false);

    importLogStore.setCurrentMod(second);
    expect(entry(first)?.isActive).toBe(false);
    expect(entry(second)).toMatchObject({ isActive: true, expanded: true });
  });
});
