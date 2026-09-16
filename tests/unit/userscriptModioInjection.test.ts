// @vitest-environment-options {"url": "https://mod.io/g/readyornot/m/alpha"}
import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";

vi.stubGlobal("GM_getValue", () => false);
vi.stubGlobal("GM_setValue", vi.fn());
vi.stubGlobal("GM_registerMenuCommand", vi.fn());
vi.stubGlobal("GM_xmlhttpRequest", vi.fn());

beforeAll(() => {
  // mod.io renders asynchronously, so the script polls for the subscribe
  // button; fake timers make each poll tick explicit.
  vi.useFakeTimers();
});

beforeEach(async () => {
  // Drain MutationObserver callbacks left over from the previous test while
  // no page matches, then drop the polls they may have queued.
  window.history.replaceState({}, "", "/");
  document.body.innerHTML = "";
  await flushMutations();
  vi.clearAllTimers();
  window.history.replaceState({}, "", "/g/readyornot/m/alpha");
});

afterEach(() => {
  window.history.replaceState({}, "", "/");
  document.body.innerHTML = "";
});

/** Let queued MutationObserver callbacks run. */
async function flushMutations() {
  for (let i = 0; i < 10; i += 1) await Promise.resolve();
}

/** Import a fresh instance so every test runs injectModIo from scratch. */
async function loadScript() {
  vi.resetModules();
  await import("../../userscript/src/main");
}

function renderSubscribeButton(classes: string) {
  document.body.innerHTML = `<div><button large="false" class="${classes}">Subscribe</button></div>`;
  return document.querySelector<HTMLButtonElement>('button[large="false"]')!;
}

function installButton(): HTMLButtonElement | null {
  return document.getElementById(
    "ronmm-subscribe-btn",
  ) as HTMLButtonElement | null;
}

describe("mod.io install button injection", () => {
  it("adds an install button that mirrors the subscribe button", async () => {
    const subscribe = renderSubscribeButton("tw-bg-primary tw-text-white");

    await loadScript();
    vi.advanceTimersByTime(200);

    const btn = installButton();
    expect(btn).not.toBeNull();
    expect(btn?.textContent).toBe("Install via RoN Mod Manager");
    expect(btn?.type).toBe("button");
    // Styling is copied so the button matches mod.io's own controls.
    expect(btn?.className).toContain("tw-bg-primary");
    expect(btn?.className).toContain("tw-cursor-pointer");
    expect(btn?.className).not.toContain("tw-opacity-50");
    expect(btn?.parentElement).toBe(subscribe.parentElement);
  });

  it("does not add a second button when one is already present", async () => {
    renderSubscribeButton("tw-mt-2");

    await loadScript();
    vi.advanceTimersByTime(200);
    vi.advanceTimersByTime(1000);

    expect(document.querySelectorAll("#ronmm-subscribe-btn")).toHaveLength(1);
  });

  it("leaves a greyed out subscribe button alone", async () => {
    renderSubscribeButton("tw-opacity-50");

    await loadScript();
    vi.advanceTimersByTime(500);

    expect(installButton()).toBeNull();
  });

  it("waits for the subscribe button to render", async () => {
    document.body.innerHTML = `<div id="host"></div>`;

    await loadScript();
    vi.advanceTimersByTime(300);
    expect(installButton()).toBeNull();

    document.getElementById("host")!.innerHTML =
      `<button large="false">Subscribe</button>`;
    vi.advanceTimersByTime(200);
    expect(installButton()).not.toBeNull();
  });

  it("re-injects after the page swaps content in place", async () => {
    renderSubscribeButton("tw-mt-2");

    await loadScript();
    vi.advanceTimersByTime(200);
    expect(installButton()).not.toBeNull();

    // SPA navigation replaces the page content without re-running the script.
    renderSubscribeButton("tw-mt-2");
    await flushMutations();
    vi.advanceTimersByTime(200);

    expect(document.querySelectorAll("#ronmm-subscribe-btn")).toHaveLength(1);
  });

  it("ignores pages that are not mod pages", async () => {
    window.history.replaceState({}, "", "/g/readyornot");
    renderSubscribeButton("tw-mt-2");

    await loadScript();
    vi.advanceTimersByTime(500);
    expect(installButton()).toBeNull();

    document.body.innerHTML += `<span>more page</span>`;
    await flushMutations();
    vi.advanceTimersByTime(500);
    expect(installButton()).toBeNull();
  });
});
