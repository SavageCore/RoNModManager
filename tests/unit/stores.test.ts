import { afterEach, describe, expect, it, vi } from "vitest";
import { get } from "svelte/store";
import { toastStore } from "../../src/lib/stores/toast";
import { modAddQueueStore } from "../../src/lib/stores/modAddQueue";
import { operationStatusStore } from "../../src/lib/stores/operationStatus";
import {
  buildMetadataRefreshDetailText,
  metadataRefreshDetailsStore,
} from "../../src/lib/stores/metadataRefreshDetailsStore";
import { manualDownloadStore } from "../../src/lib/stores/manualDownloadStore";
import { alertStore } from "../../src/lib/stores/alert";
import { infoLogStore } from "../../src/lib/stores/infoLogStore";
import { syncLogStore } from "../../src/lib/stores/syncLogStore";
import { addModpackPanelStore } from "../../src/lib/stores/addModpackPanelStore";
import {
  nexusFileSelectionStore,
  requestNexusFileSelection,
} from "../../src/lib/stores/nexusFileSelection";
import {
  pakSelectionStore,
  requestPakSelection,
} from "../../src/lib/stores/pakSelection";
import { tokenStore } from "../../src/lib/stores/token";
import {
  pendingInstall,
  pendingInstallUrl,
} from "../../src/lib/stores/pendingInstall";
import {
  DUMMY_COLLECTION_COLORS,
  DUMMY_COLLECTIONS,
  DUMMY_MOD_GROUPS,
  DUMMY_PROFILE_MODS,
  DUMMY_PROFILES,
  DUMMY_TAGS,
} from "../../src/lib/stores/incognitoMode";
import { phaseLabel } from "../../src/lib/stores/importLogStore";

afterEach(() => {
  vi.useRealTimers();
});

describe("toastStore", () => {
  it("adds toasts with default info type", () => {
    const id = toastStore.add("hello");
    const state = get(toastStore);
    expect(state.toasts.some((t) => t.id === id && t.type === "info")).toBe(
      true,
    );
    toastStore.remove(id);
  });

  it("clamps durations to the maximum", () => {
    const id = toastStore.add("long", "info", 999_999_999);
    const toast = get(toastStore).toasts.find((t) => t.id === id);
    expect(toast?.duration).toBe(3600000);
    toastStore.remove(id);
  });

  it("keeps zero-duration toasts persistent", () => {
    vi.useFakeTimers();
    const id = toastStore.add("sticky", "info", 0);
    vi.advanceTimersByTime(60_000);
    expect(get(toastStore).toasts.some((t) => t.id === id)).toBe(true);
    toastStore.remove(id);
  });

  it("auto-removes toasts after their duration", () => {
    vi.useFakeTimers();
    const id = toastStore.add("ephemeral", "success", 1000);
    expect(get(toastStore).toasts.some((t) => t.id === id)).toBe(true);
    vi.advanceTimersByTime(1001);
    expect(get(toastStore).toasts.some((t) => t.id === id)).toBe(false);
  });

  it("exposes typed helpers and remove", () => {
    const a = toastStore.success("ok");
    const b = toastStore.error("bad");
    const c = toastStore.info("note");
    const d = toastStore.warning("careful");
    const state = get(toastStore);
    expect(state.toasts.find((t) => t.id === a)?.type).toBe("success");
    expect(state.toasts.find((t) => t.id === b)?.type).toBe("error");
    expect(state.toasts.find((t) => t.id === c)?.type).toBe("info");
    expect(state.toasts.find((t) => t.id === d)?.type).toBe("warning");
    expect(state.toasts.find((t) => t.id === d)?.duration).toBe(8000);
    toastStore.remove(a);
    toastStore.remove(b);
    toastStore.remove(c);
    toastStore.remove(d);
    expect(get(toastStore).toasts.some((t) => t.id === a)).toBe(false);
  });

  it("converts errors and plain values via fromError", () => {
    const a = toastStore.fromError(new Error("boom"), 0);
    const b = toastStore.fromError("plain failure", 0);
    const state = get(toastStore);
    expect(state.toasts.find((t) => t.id === a)?.message).toBe("boom");
    expect(state.toasts.find((t) => t.id === b)?.message).toBe("plain failure");
    toastStore.remove(a);
    toastStore.remove(b);
  });
});

describe("modAddQueueStore", () => {
  it("enqueues, names, runs and completes items", () => {
    const id = modAddQueueStore.enqueue("https://example.com/mod.zip");
    modAddQueueStore.setName(id, "Example");
    // Setting the same name again leaves the item untouched.
    modAddQueueStore.setName(id, "Example");
    modAddQueueStore.markRunning(id, "Downloading");
    let item = get(modAddQueueStore).items.find((i) => i.id === id);
    expect(item?.status).toBe("running");
    expect(item?.name).toBe("Example");
    modAddQueueStore.markDone(id, "Installed");
    item = get(modAddQueueStore).items.find((i) => i.id === id);
    expect(item?.status).toBe("done");
    expect(item?.message).toBe("Installed");
    modAddQueueStore.clearHistory();
    expect(get(modAddQueueStore).items.some((i) => i.id === id)).toBe(false);
    modAddQueueStore.resetBatch();
  });

  it("tracks errors and keeps active items on clearHistory", () => {
    const failed = modAddQueueStore.enqueue("bad-input");
    const active = modAddQueueStore.enqueue("good-input");
    modAddQueueStore.markError(failed, "boom");
    modAddQueueStore.clearHistory();
    const state = get(modAddQueueStore);
    expect(state.items.some((i) => i.id === failed)).toBe(false);
    expect(state.items.some((i) => i.id === active)).toBe(true);
    modAddQueueStore.resetBatch();
    expect(get(modAddQueueStore).totalQueued).toBe(0);
  });

  it("caps history at 20 while always keeping active items", () => {
    // Drain leftovers from earlier tests (resetBatch keeps queued items).
    for (const item of get(modAddQueueStore).items) {
      modAddQueueStore.markDone(item.id, "drained");
    }
    modAddQueueStore.clearHistory();
    modAddQueueStore.resetBatch();
    const ids: string[] = [];
    for (let i = 0; i < 25; i += 1) {
      ids.push(modAddQueueStore.enqueue(`input-${i}`));
    }
    for (const id of ids) modAddQueueStore.markDone(id, "done");
    const lingering = modAddQueueStore.enqueue("still-here");
    const state = get(modAddQueueStore);
    expect(state.items).toHaveLength(21);
    expect(state.items.some((i) => i.id === lingering)).toBe(true);
    modAddQueueStore.resetBatch();
  });
});

describe("operationStatusStore", () => {
  it("clamps percent into 0-100", () => {
    operationStatusStore.updateFromProgress({
      operation: "extract",
      file: "a.pak",
      percent: 150,
      message: "",
      total_bytes: null,
      processed_bytes: null,
    });
    expect(get(operationStatusStore).percent).toBe(100);
    operationStatusStore.updateFromProgress({
      operation: "extract",
      file: "a.pak",
      percent: -5,
      message: "",
      total_bytes: null,
      processed_bytes: null,
    });
    expect(get(operationStatusStore).percent).toBe(0);
    operationStatusStore.clear();
  });

  it("auto-hides after completion", () => {
    vi.useFakeTimers();
    operationStatusStore.updateFromProgress({
      operation: "complete",
      file: "",
      percent: 100,
      message: "done",
      total_bytes: null,
      processed_bytes: null,
    });
    expect(get(operationStatusStore).visible).toBe(true);
    vi.advanceTimersByTime(5001);
    expect(get(operationStatusStore).visible).toBe(false);
  });

  it("shows temporary messages and clears them", () => {
    vi.useFakeTimers();
    operationStatusStore.setTemporaryMessage("cooldown", 2000);
    const state = get(operationStatusStore);
    expect(state.visible).toBe(true);
    expect(state.temporary).toBe(true);
    vi.advanceTimersByTime(2001);
    expect(get(operationStatusStore).visible).toBe(false);
    operationStatusStore.clear();
  });
});

describe("metadataRefreshDetailsStore", () => {
  it("orders FAILED, HIDDEN then SKIPPED with counts", () => {
    const text = buildMetadataRefreshDetailText(
      [{ name: "s", reason: "r1" }],
      [{ name: "f", reason: "r2" }],
      [{ name: "h", reason: "r3" }],
    );
    expect(text).toBe(
      "FAILED (1):\n  - f: r2\nHIDDEN (1):\n  - h: r3\nSKIPPED (1):\n  - s: r1",
    );
  });

  it("returns an empty string when there is nothing to report", () => {
    expect(buildMetadataRefreshDetailText([], [])).toBe("");
  });

  it("stores and clears details", () => {
    metadataRefreshDetailsStore.setDetails(
      [{ name: "s", reason: "x" }],
      [{ name: "f", reason: "y" }],
    );
    expect(get(metadataRefreshDetailsStore).skipped).toHaveLength(1);
    metadataRefreshDetailsStore.clear();
    expect(get(metadataRefreshDetailsStore)).toEqual({
      skipped: [],
      hidden: [],
      failed: [],
    });
  });
});

describe("manualDownloadStore", () => {
  it("dedupes by fileName or modUrl", () => {
    manualDownloadStore.clear();
    manualDownloadStore.add({
      prettyName: "A",
      fileName: "a.zip",
      modUrl: "https://m/1",
      waitId: 1,
    });
    manualDownloadStore.add({
      prettyName: "A2",
      fileName: "a.zip",
      modUrl: "https://m/2",
      waitId: 2,
    });
    manualDownloadStore.add({
      prettyName: "B",
      fileName: "b.zip",
      modUrl: "https://m/2",
      waitId: 3,
    });
    // B shares A2's modUrl, so it replaces A2 instead of appending.
    const pending = get(manualDownloadStore).pendingFiles;
    expect(pending).toHaveLength(1);
    expect(pending[0].prettyName).toBe("B");
    manualDownloadStore.add({
      prettyName: "C",
      fileName: "c.zip",
      modUrl: "https://m/3",
      waitId: 4,
    });
    expect(get(manualDownloadStore).pendingFiles).toHaveLength(2);
    expect(get(manualDownloadStore).dismissed).toBe(false);
    manualDownloadStore.clear();
  });

  it("dismisses, reopens and resets dismissed when emptied", () => {
    manualDownloadStore.clear();
    manualDownloadStore.add({
      prettyName: null,
      fileName: "a.zip",
      modUrl: "https://m/1",
      waitId: null,
    });
    manualDownloadStore.dismiss();
    expect(get(manualDownloadStore).dismissed).toBe(true);
    manualDownloadStore.reopen();
    expect(get(manualDownloadStore).dismissed).toBe(false);
    manualDownloadStore.remove("a.zip");
    expect(get(manualDownloadStore).pendingFiles).toHaveLength(0);
    expect(get(manualDownloadStore).dismissed).toBe(false);
  });
});

describe("alertStore", () => {
  it("sets typed alerts and clears to info", () => {
    alertStore.success("great");
    expect(get(alertStore)).toEqual({ message: "great", type: "success" });
    alertStore.error("bad");
    expect(get(alertStore).type).toBe("error");
    alertStore.info("note");
    expect(get(alertStore).type).toBe("info");
    alertStore.set("custom", "error");
    expect(get(alertStore)).toEqual({ message: "custom", type: "error" });
    alertStore.clear();
    expect(get(alertStore)).toEqual({ message: "", type: "info" });
  });
});

describe("infoLogStore and syncLogStore", () => {
  it("runs a start/write/finish cycle", () => {
    infoLogStore.start();
    infoLogStore.addLine("one");
    infoLogStore.finish("success");
    expect(get(infoLogStore)).toMatchObject({
      lines: ["one"],
      isOpen: true,
      isBusy: false,
      tone: "success",
    });
    infoLogStore.toggle();
    expect(get(infoLogStore).isOpen).toBe(false);
    infoLogStore.close();
    infoLogStore.clear();
    expect(get(infoLogStore).lines).toEqual([]);
  });

  it("tracks sync log lines", () => {
    syncLogStore.start();
    syncLogStore.addLine("syncing");
    syncLogStore.finish();
    expect(get(syncLogStore)).toMatchObject({
      log: ["syncing"],
      isOpen: true,
      isBusy: false,
    });
    syncLogStore.toggle();
    expect(get(syncLogStore).isOpen).toBe(false);
    syncLogStore.clear();
    expect(get(syncLogStore).log).toEqual([]);
    syncLogStore.start();
    syncLogStore.close();
    expect(get(syncLogStore).isOpen).toBe(false);
  });
});

describe("addModpackPanelStore", () => {
  it("opens, tracks activity and counts completions", () => {
    addModpackPanelStore.open("update", {
      currentVersion: "1.0",
      newVersion: "2.0",
      url: "https://pack.example/m.json",
    });
    let state = get(addModpackPanelStore);
    expect(state.isOpen).toBe(true);
    expect(state.mode).toBe("update");
    expect(state.newVersion).toBe("2.0");
    addModpackPanelStore.setActivity(true);
    addModpackPanelStore.notifyDone();
    addModpackPanelStore.notifyModInstalled();
    state = get(addModpackPanelStore);
    expect(state.hasActivity).toBe(true);
    expect(state.doneCounter).toBe(1);
    expect(state.modInstalledCounter).toBe(1);
    addModpackPanelStore.toggle();
    expect(get(addModpackPanelStore).isOpen).toBe(false);
    addModpackPanelStore.close();
  });
});

describe("selection request stores", () => {
  it("resolves nexus file selections through the store", async () => {
    const files = [{ file_id: 1, name: "a" }];
    const pending = requestNexusFileSelection(
      "Mod",
      files as unknown as Parameters<typeof requestNexusFileSelection>[1],
    );
    expect(get(nexusFileSelectionStore)?.modName).toBe("Mod");
    get(nexusFileSelectionStore)?.resolve(files as never);
    await expect(pending).resolves.toBe(files);
    nexusFileSelectionStore.set(null);
  });

  it("resolves pak selections through the store", async () => {
    const paks = [{ name: "a.pak", path: "/a.pak" }];
    const pending = requestPakSelection(
      "archive",
      paks as unknown as Parameters<typeof requestPakSelection>[1],
    );
    expect(get(pakSelectionStore)?.archiveName).toBe("archive");
    get(pakSelectionStore)?.resolve(["a.pak"]);
    await expect(pending).resolves.toEqual(["a.pak"]);
    pakSelectionStore.set(null);
  });
});

describe("simple writables", () => {
  it("toggles the token flag", () => {
    tokenStore.set(true);
    expect(get(tokenStore)).toBe(true);
    tokenStore.set(false);
    expect(get(tokenStore)).toBe(false);
  });

  it("holds pending install payloads", () => {
    pendingInstallUrl.set("https://www.nexusmods.com/readyornot/mods/1234");
    pendingInstall.set({ url: "https://example.com", fileIds: [5] });
    expect(get(pendingInstall)?.fileIds).toEqual([5]);
    pendingInstallUrl.set(null);
    pendingInstall.set(null);
    expect(get(pendingInstall)).toBeNull();
  });
});

describe("phaseLabel", () => {
  it("maps backend operations to log labels", () => {
    expect(phaseLabel("download", "Waiting for browser")).toBe(
      "Waiting for download...",
    );
    expect(phaseLabel("download", "Opening Nexus tab")).toBe(
      "Waiting for download...",
    );
    expect(phaseLabel("download", "Fetching")).toBe("Downloading...");
    expect(phaseLabel("install", "")).toBe("Preparing...");
    expect(phaseLabel("hash", "")).toBe("Verifying archive...");
    expect(phaseLabel("dedupe", "")).toBe("Checking for duplicates...");
    expect(phaseLabel("extract", "")).toBe("Extracting files...");
    expect(phaseLabel("weird-op", "")).toBe("weird-op");
  });
});

describe("incognito dummy fixtures", () => {
  it("keeps derived collections consistent", () => {
    expect(DUMMY_MOD_GROUPS.length).toBeGreaterThan(0);
    expect(DUMMY_PROFILE_MODS).toEqual(DUMMY_MOD_GROUPS.map((g) => g.name));
    for (const names of Object.values(DUMMY_COLLECTIONS)) {
      for (const name of names) {
        expect(DUMMY_PROFILE_MODS).toContain(name);
      }
    }
    expect(Object.keys(DUMMY_COLLECTION_COLORS).sort()).toEqual(
      Object.keys(DUMMY_COLLECTIONS).sort(),
    );
    expect(DUMMY_PROFILES.length).toBe(3);
    for (const mod of Object.values(DUMMY_TAGS).flat()) {
      expect(DUMMY_PROFILE_MODS).toContain(mod);
    }
  });
});
