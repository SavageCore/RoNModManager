import { describe, expect, it, vi } from "vitest";
import { get } from "svelte/store";

async function fresh<T>(path: string, seed: string): Promise<T> {
  return (await import(/* @vite-ignore */ `${path}?seed=${seed}`)) as T;
}

describe("updateCheckStore persistence", () => {
  it("loads a stored timestamp", async () => {
    localStorage.clear();
    localStorage.setItem("ronmodmanager.updateLastCheckedAt", "12345");
    const m = await fresh<{ updateCheckStore: { subscribe: unknown } }>(
      "../../src/lib/stores/updateCheck",
      "valid",
    );
    expect(get(m.updateCheckStore as never)).toBe(12345);
  });

  it("falls back to null for missing or invalid values", async () => {
    localStorage.clear();
    const empty = await fresh<{ updateCheckStore: { subscribe: unknown } }>(
      "../../src/lib/stores/updateCheck",
      "empty",
    );
    expect(get(empty.updateCheckStore as never)).toBeNull();

    localStorage.setItem("ronmodmanager.updateLastCheckedAt", "nope");
    const invalid = await fresh<{ updateCheckStore: { subscribe: unknown } }>(
      "../../src/lib/stores/updateCheck",
      "invalid",
    );
    expect(get(invalid.updateCheckStore as never)).toBeNull();

    localStorage.setItem("ronmodmanager.updateLastCheckedAt", "0");
    const nonPositive = await fresh<{
      updateCheckStore: { subscribe: unknown };
    }>("../../src/lib/stores/updateCheck", "non-positive");
    expect(get(nonPositive.updateCheckStore as never)).toBeNull();
  });

  it("marks, sets and clears the timestamp", async () => {
    localStorage.clear();
    const m = await fresh<{
      updateCheckStore: {
        subscribe: unknown;
        markChecked: () => number;
        setLastCheckedAt: (v: number | null) => void;
        clear: () => void;
      };
    }>("../../src/lib/stores/updateCheck", "rw");
    const now = m.updateCheckStore.markChecked();
    expect(typeof now).toBe("number");
    expect(localStorage.getItem("ronmodmanager.updateLastCheckedAt")).toBe(
      String(now),
    );
    m.updateCheckStore.setLastCheckedAt(999);
    expect(get(m.updateCheckStore as never)).toBe(999);
    m.updateCheckStore.clear();
    expect(get(m.updateCheckStore as never)).toBeNull();
    expect(
      localStorage.getItem("ronmodmanager.updateLastCheckedAt"),
    ).toBeNull();
  });
});

describe("modSortOrder persistence", () => {
  it("falls back to the default for invalid values", async () => {
    localStorage.clear();
    localStorage.setItem("ronmodmanager.modSortOrder", "bogus");
    const m = await fresh<{ modSortOrder: { subscribe: unknown } }>(
      "../../src/lib/stores/modSortOrder",
      "invalid",
    );
    expect(get(m.modSortOrder as never)).toBe("alpha-asc");
  });

  it("loads and persists valid values", async () => {
    localStorage.clear();
    localStorage.setItem("ronmodmanager.modSortOrder", "date-desc");
    const m = await fresh<{
      modSortOrder: { subscribe: unknown; set: (v: string) => void };
    }>("../../src/lib/stores/modSortOrder", "valid");
    expect(get(m.modSortOrder as never)).toBe("date-desc");
    m.modSortOrder.set("files-asc");
    expect(localStorage.getItem("ronmodmanager.modSortOrder")).toBe(
      "files-asc",
    );
  });
});

describe("boolean localStorage flags", () => {
  it("reads showBroken from storage", async () => {
    localStorage.clear();
    localStorage.setItem("ronmodmanager.showBroken", "true");
    const m = await fresh<{ showBroken: { subscribe: unknown } }>(
      "../../src/lib/stores/showBroken",
      "true",
    );
    expect(get(m.showBroken as never)).toBe(true);
  });

  it("defaults ue4ssBannerDismissed to false", async () => {
    localStorage.clear();
    const m = await fresh<{ ue4ssBannerDismissed: { subscribe: unknown } }>(
      "../../src/lib/stores/ue4ssBannerDismissed",
      "default",
    );
    expect(get(m.ue4ssBannerDismissed as never)).toBe(false);
  });
});

describe("modToggleState", () => {
  it("loads saved JSON and supports remove/clear helpers", async () => {
    localStorage.clear();
    localStorage.setItem(
      "modToggleState",
      JSON.stringify({ a: true, b: false }),
    );
    const m = await fresh<{
      modToggleState: { subscribe: unknown; set: (v: unknown) => void };
      modToggleStateHelpers: { remove: (n: string) => void; clear: () => void };
    }>("../../src/lib/stores/modState", "valid");
    expect(get(m.modToggleState as never)).toEqual({ a: true, b: false });
    m.modToggleStateHelpers.remove("a");
    expect(get(m.modToggleState as never)).toEqual({ b: false });
    m.modToggleStateHelpers.clear();
    expect(get(m.modToggleState as never)).toEqual({});
  });

  it("ignores corrupt JSON", async () => {
    localStorage.clear();
    localStorage.setItem("modToggleState", "{broken");
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const m = await fresh<{ modToggleState: { subscribe: unknown } }>(
      "../../src/lib/stores/modState",
      "corrupt",
    );
    expect(get(m.modToggleState as never)).toEqual({});
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });
});
