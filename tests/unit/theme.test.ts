import { afterEach, describe, expect, it, vi } from "vitest";
import { applyThemeClass, initTheme } from "../../src/lib/theme";

function stubMatchMedia(matches: boolean) {
  const listeners = new Map<string, Set<() => void>>();
  const mql = {
    matches,
    addEventListener: vi.fn((event: string, cb: () => void) => {
      let set = listeners.get(event);
      if (!set) {
        set = new Set();
        listeners.set(event, set);
      }
      set.add(cb);
    }),
    removeEventListener: vi.fn((event: string, cb: () => void) => {
      listeners.get(event)?.delete(cb);
    }),
    dispatch: () => {
      listeners.get("change")?.forEach((cb) => cb());
    },
  };
  Object.defineProperty(window, "matchMedia", {
    value: vi.fn(() => mql),
    configurable: true,
    writable: true,
  });
  return mql;
}

afterEach(() => {
  document.documentElement.removeAttribute("data-theme");
  document.documentElement.classList.remove("dark");
  vi.restoreAllMocks();
});

describe("applyThemeClass", () => {
  it("applies the light theme", () => {
    applyThemeClass("light");
    expect(document.documentElement.getAttribute("data-theme")).toBe(
      "ron-light",
    );
    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });

  it("applies the dark theme", () => {
    applyThemeClass("dark");
    expect(document.documentElement.getAttribute("data-theme")).toBe(
      "ron-dark",
    );
    expect(document.documentElement.classList.contains("dark")).toBe(true);
  });

  it("follows the OS preference in system mode (dark)", () => {
    stubMatchMedia(true);
    applyThemeClass("system");
    expect(document.documentElement.getAttribute("data-theme")).toBe(
      "ron-dark",
    );
    expect(document.documentElement.classList.contains("dark")).toBe(true);
  });

  it("follows the OS preference in system mode (light)", () => {
    stubMatchMedia(false);
    applyThemeClass("system");
    expect(document.documentElement.getAttribute("data-theme")).toBe(
      "ron-light",
    );
    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });
});

describe("initTheme", () => {
  it("returns a noop cleanup for a fixed mode", () => {
    stubMatchMedia(false);
    const cleanup = initTheme("light");
    expect(document.documentElement.getAttribute("data-theme")).toBe(
      "ron-light",
    );
    expect(typeof cleanup).toBe("function");
    cleanup();
  });

  it("subscribes to OS changes in system mode and cleans up", () => {
    const mql = stubMatchMedia(false);
    const cleanup = initTheme("system");
    expect(document.documentElement.getAttribute("data-theme")).toBe(
      "ron-light",
    );

    (mql as { matches: boolean }).matches = true;
    mql.dispatch();
    expect(document.documentElement.getAttribute("data-theme")).toBe(
      "ron-dark",
    );

    cleanup();
    expect(mql.removeEventListener).toHaveBeenCalledWith(
      "change",
      expect.any(Function),
    );
  });
});
