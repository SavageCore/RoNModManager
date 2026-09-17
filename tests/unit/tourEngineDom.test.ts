import { get } from "svelte/store";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { updateConfig, goto, currentPath } = vi.hoisted(() => ({
  updateConfig: vi.fn(async () => undefined),
  goto: vi.fn(async () => undefined),
  currentPath: { value: "/mods" },
}));

vi.mock("$app/navigation", () => ({ goto }));
vi.mock("$app/stores", () => ({
  page: {
    subscribe: (run: (value: unknown) => void) => {
      run({ url: new URL(`http://localhost${currentPath.value}`) });
      return () => {};
    },
  },
}));
vi.mock("../../src/lib/api/commands", () => ({ updateConfig }));

type MockStep = {
  id: string;
  title: string;
  body: string;
  setup?: boolean;
  route?: string;
  target?: string;
  targetTimeoutMs?: number;
  placement?: "above" | "below" | "left" | "right";
  bullets?: string[];
  showRing?: boolean;
  pulseTarget?: boolean;
  autoAdvanceMs?: number;
  advanceOnReady?: boolean;
  nextAction?: string;
  resolveTarget?: () => Element | null;
  enter?: () => Promise<void>;
  leave?: () => Promise<void>;
  ready?: () => Promise<boolean>;
};

async function loadEngine(steps: MockStep[]) {
  vi.resetModules();
  vi.doMock("../../src/lib/tour/steps", () => ({ TOUR_STEPS: steps }));
  return await import("../../src/lib/tour/tourEngine");
}

const RECT = {
  top: 100,
  left: 100,
  width: 120,
  height: 40,
  bottom: 140,
  right: 220,
  x: 100,
  y: 100,
  toJSON: () => ({}),
} as DOMRect;

function mountTarget(anchor: string): HTMLElement {
  const el = document.createElement("div");
  el.setAttribute("data-tour", anchor);
  el.getBoundingClientRect = () => RECT;
  document.body.appendChild(el);
  return el;
}

beforeEach(() => {
  vi.clearAllMocks();
  currentPath.value = "/mods";
  document.body.innerHTML = "";
});

afterEach(() => {
  vi.useRealTimers();
});

describe("tourEngine target tracking", () => {
  it("records the target rect and stops waiting once it appears", async () => {
    mountTarget("hit");
    const engine = await loadEngine([
      { id: "s", title: "S", body: "b", target: '[data-tour="hit"]' },
    ]);

    await engine.startTour();

    const state = get(engine.tourState);
    expect(state.waitingForTarget).toBe(false);
    expect(state.rect?.top).toBe(100);
    expect(state.rect?.width).toBe(120);
    await engine.closeTour();
    expect(get(engine.tourState).rect).toBeNull();
  });

  it("flags a target that never appears", async () => {
    const engine = await loadEngine([
      {
        id: "s",
        title: "S",
        body: "b",
        target: '[data-tour="never"]',
        targetTimeoutMs: 50,
      },
    ]);

    await engine.startTour();

    const state = get(engine.tourState);
    expect(state.waitingForTarget).toBe(true);
    expect(state.rect).toBeNull();
    await engine.closeTour();
  });

  it("re-measures on resize so the ring follows the target", async () => {
    const el = mountTarget("hit");
    const engine = await loadEngine([
      { id: "s", title: "S", body: "b", target: '[data-tour="hit"]' },
    ]);
    await engine.startTour();

    el.getBoundingClientRect = () => ({ ...RECT, top: 300 }) as DOMRect;
    window.dispatchEvent(new Event("resize"));
    document.dispatchEvent(new Event("scroll"));

    expect(get(engine.tourState).rect?.top).toBe(300);
    await engine.closeTour();
  });

  it("uses a custom resolveTarget for dynamic rows", async () => {
    const engine = await loadEngine([
      {
        id: "s",
        title: "S",
        body: "b",
        target: '[data-tour="never"]',
        resolveTarget: () => document.querySelector('[data-tour="row"]'),
        targetTimeoutMs: 200,
      },
    ]);

    const starting = engine.startTour();
    const row = document.createElement("li");
    row.setAttribute("data-tour", "row");
    row.getBoundingClientRect = () => RECT;
    document.body.appendChild(row);
    await starting;

    expect(get(engine.tourState).waitingForTarget).toBe(false);
    expect(get(engine.tourState).rect).not.toBeNull();
    await engine.closeTour();
  });

  it("reports a step whose ready condition never happens", async () => {
    const engine = await loadEngine([
      {
        id: "s",
        title: "S",
        body: "b",
        ready: async () => false,
      },
    ]);

    await engine.startTour();

    await vi.waitFor(() => {
      expect(get(engine.tourState).ready).toBe(true);
    });
    expect(get(engine.tourState).readyTimedOut).toBe(true);
    await engine.closeTour();
  });

  it("marks a satisfied ready condition as clean", async () => {
    const engine = await loadEngine([
      { id: "s", title: "S", body: "b", ready: async () => true },
    ]);

    await engine.startTour();

    await vi.waitFor(() => {
      expect(get(engine.tourState).readyTimedOut).toBe(false);
    });
    expect(get(engine.tourState).ready).toBe(true);
    await engine.closeTour();
  });
});

describe("tourEngine navigation", () => {
  it("navigates when a step needs another page", async () => {
    mountTarget("hit");
    const engine = await loadEngine([
      {
        id: "s",
        title: "S",
        body: "b",
        route: "/settings",
        target: '[data-tour="hit"]',
      },
    ]);

    await engine.startTour();

    expect(goto).toHaveBeenCalledWith("/settings");
    await engine.closeTour();
  });

  it("does not navigate when the step is already on the right page", async () => {
    currentPath.value = "/settings";
    mountTarget("hit");
    const engine = await loadEngine([
      {
        id: "s",
        title: "S",
        body: "b",
        route: "/settings",
        target: '[data-tour="hit"]',
      },
    ]);

    await engine.startTour();

    expect(goto).not.toHaveBeenCalled();
    await engine.closeTour();
  });

  it("re-arms a step's own timer after Back", async () => {
    const engine = await loadEngine([
      { id: "a", title: "A", body: "a", autoAdvanceMs: 40 },
      { id: "b", title: "B", body: "b" },
    ]);

    await engine.startTour();
    await engine.nextStep();
    expect(get(engine.tourState).index).toBe(1);
    await engine.prevStep();
    expect(get(engine.tourState).index).toBe(0);

    // Back must not leave the user stranded on a step that paces itself.
    await vi.waitFor(() => {
      expect(get(engine.tourState).index).toBe(1);
    });
    await engine.closeTour();
  });

  it("ignores Next and Back while idle", async () => {
    const engine = await loadEngine([
      { id: "a", title: "A", body: "a" },
      { id: "b", title: "B", body: "b" },
    ]);

    await engine.nextStep();
    await engine.prevStep();

    expect(get(engine.tourState).running).toBe(false);
    expect(get(engine.tourState).index).toBe(0);
  });

  it("marks the tour complete even when closed without starting", async () => {
    const engine = await loadEngine([{ id: "a", title: "A", body: "a" }]);

    await engine.closeTour();

    expect(updateConfig).toHaveBeenCalledWith({ tutorial_complete: true });
  });
});

describe("tourEngine next action", () => {
  it("enables Next and does the work when the user presses it", async () => {
    const engine = await loadEngine([
      {
        id: "a",
        title: "A",
        body: "a",
        // Never satisfied on its own: Next is the only way forward.
        ready: async () => false,
        nextAction: "mods:open-add-mod",
      },
      { id: "b", title: "B", body: "b" },
    ]);
    // After loadEngine, so the registry instance is the engine's.
    const registry = await import("../../src/lib/tour/registry");
    const openAddMod = vi.fn();
    registry.registerTourActions("mods", { "open-add-mod": openAddMod });

    await engine.startTour();
    expect(get(engine.tourState).ready).toBe(true);

    await engine.nextStep();

    expect(openAddMod).toHaveBeenCalledTimes(1);
    expect(get(engine.tourState).index).toBe(1);
    await engine.closeTour();
  });

  it("does not repeat the action when the user did it themselves", async () => {
    const engine = await loadEngine([
      {
        id: "a",
        title: "A",
        body: "a",
        ready: async () => true,
        advanceOnReady: true,
        nextAction: "mods:submit-add-mod",
      },
      { id: "b", title: "B", body: "b" },
    ]);
    const registry = await import("../../src/lib/tour/registry");
    const submit = vi.fn();
    registry.registerTourActions("mods", { "submit-add-mod": submit });

    await engine.startTour();

    await vi.waitFor(() => {
      expect(get(engine.tourState).index).toBe(1);
    });
    expect(submit).not.toHaveBeenCalled();
    await engine.closeTour();
  });
});

describe("tourEngine auto advance", () => {
  it("moves on by itself once a gated action has happened", async () => {
    const engine = await loadEngine([
      {
        id: "a",
        title: "A",
        body: "a",
        ready: async () => true,
        advanceOnReady: true,
      },
      { id: "b", title: "B", body: "b" },
    ]);

    await engine.startTour();

    await vi.waitFor(() => {
      expect(get(engine.tourState).index).toBe(1);
    });
    expect(get(engine.currentStep)?.id).toBe("b");
    await engine.closeTour();
  });

  it("advances even when ready resolves before the step finished setting up", async () => {
    const engine = await loadEngine([
      {
        id: "a",
        title: "A",
        body: "a",
        target: '[data-tour="hit"]',
        enter: () => new Promise((resolve) => setTimeout(resolve, 40)),
        ready: async () => true,
        advanceOnReady: true,
      },
      { id: "b", title: "B", body: "b" },
    ]);
    mountTarget("hit");

    await engine.startTour();

    await vi.waitFor(() => {
      expect(get(engine.tourState).index).toBe(1);
    });
    await engine.closeTour();
  });

  it("advances even when ready resolves before the step finished setting up", async () => {
    mountTarget("hit");
    const engine = await loadEngine([
      {
        id: "a",
        title: "A",
        body: "a",
        target: '[data-tour="hit"]',
        enter: () => new Promise((resolve) => setTimeout(resolve, 40)),
        ready: async () => true,
        advanceOnReady: true,
      },
      { id: "b", title: "B", body: "b" },
    ]);

    await engine.startTour();

    await vi.waitFor(() => {
      expect(get(engine.tourState).index).toBe(1);
    });
    await engine.closeTour();
  });

  it("stays put when the gated action never happens", async () => {
    const engine = await loadEngine([
      {
        id: "a",
        title: "A",
        body: "a",
        ready: async () => false,
        advanceOnReady: true,
      },
      { id: "b", title: "B", body: "b" },
    ]);

    await engine.startTour();

    await vi.waitFor(() => {
      expect(get(engine.tourState).ready).toBe(true);
    });
    expect(get(engine.tourState).readyTimedOut).toBe(true);
    expect(get(engine.tourState).index).toBe(0);
    await engine.closeTour();
  });

  it("advances on its own after a page-transition step", async () => {
    const engine = await loadEngine([
      { id: "nav", title: "Nav", body: "n", autoAdvanceMs: 20 },
      { id: "next", title: "Next", body: "n" },
    ]);

    await engine.startTour();

    await vi.waitFor(() => {
      expect(get(engine.tourState).index).toBe(1);
    });
    await engine.closeTour();
  });

  it("cancels a pending auto advance when the tour is closed", async () => {
    const engine = await loadEngine([
      { id: "nav", title: "Nav", body: "n", autoAdvanceMs: 30 },
      { id: "next", title: "Next", body: "n" },
    ]);

    await engine.startTour();
    await engine.closeTour();
    await new Promise((resolve) => setTimeout(resolve, 80));

    expect(get(engine.tourState).running).toBe(false);
    expect(get(engine.tourState).index).toBe(0);
  });

  it("can cut the dim around a target without ringing it", async () => {
    mountTarget("hit");
    const engine = await loadEngine([
      {
        id: "s",
        title: "S",
        body: "b",
        target: '[data-tour="hit"]',
        showRing: false,
      },
    ]);

    await engine.startTour();

    expect(get(engine.tourState).rect).not.toBeNull();
    expect(get(engine.tourState).showRing).toBe(false);
    await engine.closeTour();
  });

  it("waits for the user on Back even when the step usually advances", async () => {
    let armed = false;
    const engine = await loadEngine([
      {
        id: "a",
        title: "A",
        body: "a",
        ready: async () => armed,
        advanceOnReady: true,
      },
      { id: "b", title: "B", body: "b" },
    ]);

    await engine.startTour();
    armed = true;
    await engine.nextStep();
    await engine.prevStep();
    await new Promise((resolve) => setTimeout(resolve, 30));

    expect(get(engine.tourState).index).toBe(0);
    expect(get(engine.currentStep)?.id).toBe("a");
    await engine.closeTour();
  });

  it("keeps going when a step fails to set up", async () => {
    const engine = await loadEngine([
      {
        id: "a",
        title: "A",
        body: "a",
        enter: async () => {
          throw new Error("page not ready");
        },
      },
      { id: "b", title: "B", body: "b" },
      { id: "c", title: "C", body: "c" },
    ]);
    vi.spyOn(console, "error").mockImplementation(() => {});

    await engine.startTour();
    // A thrown setup used to leave the engine busy, which refuses every Next
    // and Back and strands the user on that card for good.
    expect(get(engine.tourState).busy).toBe(false);

    await engine.nextStep();
    expect(get(engine.tourState).index).toBe(1);
    await engine.prevStep();
    expect(get(engine.tourState).index).toBe(0);
    await engine.closeTour();
  });

  it("lands on the last step's page before the tour ends", async () => {
    const engine = await loadEngine([
      { id: "a", title: "A", body: "a" },
      { id: "b", title: "B", body: "b", route: "/mods" },
    ]);
    currentPath.value = "/settings";

    await engine.startTour();
    await engine.nextStep();
    expect(goto).toHaveBeenCalledWith("/mods");

    await engine.nextStep();
    expect(get(engine.tourState).running).toBe(false);
  });
});

describe("tourEngine first-run setup", () => {
  const firstRunSteps = (): MockStep[] => [
    { id: "welcome", title: "W", body: "w" },
    {
      id: "setup-a",
      title: "SA",
      body: "sa",
      setup: true,
      target: '[data-tour="hit"]',
    },
    {
      id: "setup-b",
      title: "SB",
      body: "sb",
      setup: true,
      target: '[data-tour="hit"]',
    },
    { id: "app", title: "App", body: "app" },
  ];

  it("runs the setup cards on a first launch, and drops them from a replay", async () => {
    mountTarget("hit");
    const first = await loadEngine(firstRunSteps());

    await first.startTour({ setup: true });

    expect(get(first.tourState).total).toBe(4);
    expect(get(first.tourState).setupPending).toBe(true);

    const replay = await loadEngine(firstRunSteps());
    await replay.startTour();

    expect(get(replay.tourState).total).toBe(2);
    expect(get(replay.tourState).setupPending).toBe(false);
    expect(get(replay.currentStep)?.id).toBe("welcome");
    await replay.closeTour();
  });

  it("holds the user on the setup cards until they are past them", async () => {
    mountTarget("hit");
    const engine = await loadEngine(firstRunSteps());
    await engine.startTour({ setup: true });

    expect(get(engine.tourState).setupPending).toBe(true);

    // Skip tour and Escape both land here, and neither may cut setup short.
    await engine.closeTour();
    expect(get(engine.tourState).running).toBe(true);

    await engine.nextStep();
    await engine.closeTour();
    expect(get(engine.tourState).running).toBe(true);

    // Past the last setup card the tour is the user's to close again.
    await engine.nextStep();
    await engine.nextStep();
    expect(get(engine.tourState).setupPending).toBe(false);

    await engine.closeTour();
    expect(get(engine.tourState).running).toBe(false);
    expect(updateConfig).toHaveBeenCalledWith({ tutorial_complete: true });
  });

  it("moves past the setup cards when the user defers setup", async () => {
    mountTarget("hit");
    const engine = await loadEngine(firstRunSteps());
    await engine.startTour({ setup: true });
    await engine.nextStep();

    await engine.skipSetupSteps();

    // Straight to the app tour, with the tour cancellable again.
    expect(get(engine.currentStep)?.id).toBe("app");
    expect(get(engine.tourState).index).toBe(3);
    expect(get(engine.tourState).setupPending).toBe(false);
    await engine.closeTour();
    expect(get(engine.tourState).running).toBe(false);
  });

  it("has nothing to skip when the run has no setup cards", async () => {
    const engine = await loadEngine([
      { id: "a", title: "A", body: "a" },
      { id: "b", title: "B", body: "b" },
    ]);
    await engine.startTour();

    await engine.skipSetupSteps();

    expect(get(engine.tourState).index).toBe(0);
    expect(get(engine.currentStep)?.id).toBe("a");
    await engine.closeTour();
  });

  it("defers setup when the user picks Set up later", async () => {
    mountTarget("hit");
    const engine = await loadEngine(firstRunSteps());
    await engine.startTour({ setup: true });
    const registry = await import("../../src/lib/tour/registry");
    const settle = vi.fn(async () => {});
    registry.registerTourActions("setup", { defer: settle });

    await engine.deferSetup();

    // The surface that owns setup settles it, and the run carries on past the
    // setup cards with the tour cancellable again.
    expect(settle).toHaveBeenCalledTimes(1);
    expect(get(engine.currentStep)?.id).toBe("app");
    expect(get(engine.tourState).setupPending).toBe(false);
    await engine.closeTour();
    expect(get(engine.tourState).running).toBe(false);
  });

  it("stays on the card when the control refuses the action", async () => {
    const engine = await loadEngine([
      { id: "a", title: "A", body: "a", nextAction: "setup:save-modio" },
      { id: "b", title: "B", body: "b" },
    ]);
    const registry = await import("../../src/lib/tour/registry");
    const refused = vi.fn(async () => false);
    registry.registerTourActions("setup", { "save-modio": refused });

    await engine.startTour();
    await engine.nextStep();

    // The dialog is still on screen with its own error: moving the card on
    // would leave the user looking at a page nobody explained.
    expect(refused).toHaveBeenCalledTimes(1);
    expect(get(engine.tourState).index).toBe(0);
    expect(get(engine.currentStep)?.id).toBe("a");
    await engine.closeTour();
  });
});
