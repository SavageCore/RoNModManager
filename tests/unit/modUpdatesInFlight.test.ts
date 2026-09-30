import { describe, expect, it, vi } from "vitest";

const checkModUpdates = vi.fn<() => Promise<unknown[]>>();
vi.mock("../../src/lib/api/commands", () => ({ checkModUpdates }));

async function fresh<T>(seed: string): Promise<T> {
  return (await import(
    /* @vite-ignore */ `../../src/lib/stores/modUpdates?seed=${seed}`
  )) as T;
}

type ModUpdatesModule = {
  checkModUpdatesOnce: () => Promise<Record<string, unknown>>;
};

const update = (archive: string) => ({
  archiveName: archive,
  source: "nexus",
  currentVersion: "1.0",
  latestVersion: "2.0",
  updateAvailable: true,
});

describe("checkModUpdatesOnce", () => {
  it("collapses concurrent callers onto a single backend check", async () => {
    checkModUpdates.mockReset();
    let resolve!: (value: unknown[]) => void;
    checkModUpdates.mockReturnValue(
      new Promise((r) => {
        resolve = r;
      }),
    );
    const m = await fresh<ModUpdatesModule>("collapse");

    // The layout's startup check and the Mods page mount both call in before
    // the first pass resolves.
    const first = m.checkModUpdatesOnce();
    const second = m.checkModUpdatesOnce();
    const third = m.checkModUpdatesOnce();

    expect(checkModUpdates).toHaveBeenCalledTimes(1);
    resolve([update("a.zip"), update("b.zip")]);
    const results = await Promise.all([first, second, third]);

    // Every caller gets the same filtered result: only flagged updates.
    for (const r of results) {
      expect(Object.keys(r).sort()).toEqual(["a.zip", "b.zip"]);
    }
  });

  it("drops mods with no update available", async () => {
    checkModUpdates.mockReset();
    checkModUpdates.mockResolvedValue([
      update("stale.zip"),
      { ...update("current.zip"), updateAvailable: false },
    ]);
    const m = await fresh<ModUpdatesModule>("filter");

    const result = await m.checkModUpdatesOnce();
    expect(Object.keys(result)).toEqual(["stale.zip"]);
  });

  it("starts a fresh pass once the previous one settles", async () => {
    checkModUpdates.mockReset();
    checkModUpdates.mockResolvedValue([]);
    const m = await fresh<ModUpdatesModule>("reset");

    await m.checkModUpdatesOnce();
    await m.checkModUpdatesOnce();

    // The in-flight slot is released on settle, so this is not collapsed to 1.
    expect(checkModUpdates).toHaveBeenCalledTimes(2);
  });

  it("releases the in-flight slot when a pass rejects", async () => {
    checkModUpdates.mockReset();
    checkModUpdates.mockRejectedValueOnce(new Error("network down"));
    const m = await fresh<ModUpdatesModule>("reject");

    await expect(m.checkModUpdatesOnce()).rejects.toThrow("network down");

    checkModUpdates.mockResolvedValue([update("recovered.zip")]);
    const result = await m.checkModUpdatesOnce();
    expect(Object.keys(result)).toEqual(["recovered.zip"]);
  });
});
