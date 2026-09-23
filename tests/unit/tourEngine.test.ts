import { get } from "svelte/store";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { updateConfig, goto, steps } = vi.hoisted(() => {
  const steps = [
    {
      id: "a",
      title: "A",
      body: "body a",
      leave: vi.fn<() => Promise<void>>(async () => {}),
      ready: vi.fn<() => Promise<boolean>>(async () => true),
    },
    {
      id: "b",
      title: "B",
      body: "body b",
      enter: vi.fn<
        (ctx: { call: (name: string) => Promise<void> }) => Promise<void>
      >(async (ctx: { call: (name: string) => Promise<void> }) => {
        await ctx.call("mods:open-add-mod");
      }),
      leave: vi.fn<() => Promise<void>>(async () => {}),
      ready: vi.fn<() => Promise<boolean>>(async () => true),
    },
    {
      id: "c",
      title: "C",
      body: "body c",
      leave: vi.fn<() => Promise<void>>(async () => {}),
    },
  ];
  return {
    updateConfig: vi.fn<(patch: Record<string, unknown>) => Promise<void>>(
      async () => undefined,
    ),
    goto: vi.fn<(url: string) => Promise<void>>(async () => undefined),
    steps,
  };
});

vi.mock("$app/navigation", () => ({ goto }));
vi.mock("$app/stores", () => ({
  page: {
    subscribe: (run: (value: unknown) => void) => {
      run({ url: new URL("http://localhost/mods") });
      return () => {};
    },
  },
}));
vi.mock("../../src/lib/api/commands", () => ({ updateConfig }));
vi.mock("../../src/lib/tour/steps", () => ({ TOUR_STEPS: steps }));

async function loadEngine() {
  vi.resetModules();
  return await import("../../src/lib/tour/tourEngine");
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("tourEngine", () => {
  it("starts on the first step", async () => {
    const engine = await loadEngine();
    await engine.startTour();
    const state = get(engine.tourState);
    expect(state.running).toBe(true);
    expect(state.index).toBe(0);
    expect(state.total).toBe(steps.length);
    expect(get(engine.currentStep)?.id).toBe("a");
    await engine.closeTour();
  });

  it("advances through steps and closes after the last one", async () => {
    const engine = await loadEngine();
    await engine.startTour();
    await engine.nextStep();
    expect(get(engine.tourState).index).toBe(1);
    await engine.nextStep();
    expect(get(engine.tourState).index).toBe(2);
    await engine.nextStep();
    expect(get(engine.tourState).running).toBe(false);
    expect(updateConfig).toHaveBeenCalledWith({ tutorial_complete: true });
  });

  it("ignores Back on the first step", async () => {
    const engine = await loadEngine();
    await engine.startTour();
    await engine.prevStep();
    expect(get(engine.tourState).index).toBe(0);
    await engine.closeTour();
  });

  it("re-enters the previous step on Back", async () => {
    const engine = await loadEngine();
    await engine.startTour();
    await engine.nextStep();
    await engine.prevStep();
    expect(get(engine.tourState).index).toBe(0);
    expect(get(engine.currentStep)?.id).toBe("a");
    await engine.closeTour();
  });

  it("runs a step's enter through the action registry", async () => {
    const engine = await loadEngine();
    const registry = await import("../../src/lib/tour/registry");
    const openAddMod = vi.fn<() => unknown>();
    registry.registerTourActions("mods", { "open-add-mod": openAddMod });

    await engine.startTour();
    await engine.nextStep();

    expect(openAddMod).toHaveBeenCalledTimes(1);
    await engine.closeTour();
  });

  it("leaves the current step when closing, and marks the tour complete", async () => {
    const engine = await loadEngine();
    await engine.startTour();
    await engine.nextStep();
    await engine.closeTour();

    expect(steps[1].leave).toHaveBeenCalledTimes(1);
    expect(steps[2].leave).not.toHaveBeenCalled();
    // The step being stepped away from is left before the next one is entered.
    expect(steps[0].leave.mock.invocationCallOrder[0]).toBeLessThan(
      steps[1].leave.mock.invocationCallOrder[0],
    );
    expect(get(engine.tourState).running).toBe(false);
    expect(get(engine.currentStep)).toBeNull();
    expect(updateConfig).toHaveBeenLastCalledWith({ tutorial_complete: true });
  });

  it("replaying clears the completion flag first", async () => {
    const engine = await loadEngine();
    await engine.clearTutorialComplete();
    expect(updateConfig).toHaveBeenCalledWith({ tutorial_complete: false });
  });
});
