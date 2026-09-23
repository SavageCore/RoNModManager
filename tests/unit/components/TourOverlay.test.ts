import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { updateConfig, goto } = vi.hoisted(() => ({
  updateConfig: vi.fn<(patch: Record<string, unknown>) => Promise<void>>(
    async () => undefined,
  ),
  goto: vi.fn<(url: string) => Promise<void>>(async () => undefined),
}));

vi.mock("$app/navigation", () => ({ goto }));
vi.mock("$app/stores", () => ({
  page: {
    subscribe: (run: (value: unknown) => void) => {
      run({ url: new URL("http://localhost/mods") });
      return () => {};
    },
  },
}));
vi.mock("../../../src/lib/api/commands", () => ({ updateConfig }));
vi.mock("../../../src/lib/tour/steps", () => ({
  TOUR_STEPS: [
    { id: "a", title: "Step A", body: "Body one" },
    {
      id: "b",
      title: "Step B",
      body: "Body two",
      target: '[data-tour="mods-add-mod"]',
    },
    {
      id: "c",
      title: "Step C",
      body: "Body three\n\nA second paragraph",
      bullets: ["First bullet", "Second bullet"],
      placement: "left",
      pulseTarget: true,
      target: '[data-tour="mods-add-mod"]',
    },
  ],
}));

import TourOverlay from "../../../src/lib/components/TourOverlay.svelte";
import {
  closeTour,
  currentStep,
  startTour,
  tourState,
} from "../../../src/lib/tour/tourEngine";
import { get } from "svelte/store";

function addTarget(): HTMLElement {
  const target = document.createElement("button");
  target.setAttribute("data-tour", "mods-add-mod");
  target.getBoundingClientRect = () =>
    ({
      top: 100,
      left: 100,
      width: 120,
      height: 40,
      bottom: 140,
      right: 220,
      x: 100,
      y: 100,
      toJSON: () => ({}),
    }) as DOMRect;
  document.body.appendChild(target);
  return target;
}

beforeEach(async () => {
  vi.clearAllMocks();
  await closeTour();
  document.body.innerHTML = "";
  addTarget();
});

describe("TourOverlay", () => {
  it("renders nothing while the tour is idle", () => {
    render(TourOverlay);
    expect(screen.queryByText("Body one")).toBeNull();
    expect(get(tourState).running).toBe(false);
  });

  it("shows the current step with progress and a disabled Back on step one", async () => {
    render(TourOverlay);
    await startTour();

    expect(screen.getByText("Step A")).not.toBeNull();
    expect(screen.getByText("Body one")).not.toBeNull();
    expect(screen.getByText("1 of 3")).not.toBeNull();
    expect((screen.getByText("Back") as HTMLButtonElement).disabled).toBe(true);
  });

  it("steps forward on Next", async () => {
    render(TourOverlay);
    await startTour();

    await fireEvent.click(screen.getByText("Next"));

    expect(screen.getByText("Body two")).not.toBeNull();
    expect(screen.getByText("2 of 3")).not.toBeNull();
    expect(screen.getByText("Next")).not.toBeNull();
  });

  it("renders bullet lists and shows Finish on the last step", async () => {
    render(TourOverlay);
    await startTour();

    await fireEvent.click(screen.getByText("Next"));
    await fireEvent.click(screen.getByText("Next"));

    expect(screen.getByText("Body three")).not.toBeNull();
    expect(screen.getByText("A second paragraph")).not.toBeNull();
    expect(screen.getByText("First bullet")).not.toBeNull();
    expect(screen.getByText("Second bullet")).not.toBeNull();
    expect(screen.getByText("Finish")).not.toBeNull();
  });

  it("places the card beside the target when a step asks for it", async () => {
    const target = document.querySelector('[data-tour="mods-add-mod"]');
    const { container } = render(TourOverlay);
    await startTour();
    await fireEvent.click(screen.getByText("Next"));
    await fireEvent.click(screen.getByText("Next"));

    const card = container.querySelector(".tour-card") as HTMLElement;
    target!.getBoundingClientRect = () =>
      ({
        top: 300,
        left: 800,
        width: 120,
        height: 40,
        bottom: 340,
        right: 920,
        x: 800,
        y: 300,
        toJSON: () => ({}),
      }) as DOMRect;
    window.dispatchEvent(new Event("resize"));

    // 800 - CARD_WIDTH (380) - GAP (14)
    await vi.waitFor(() => {
      expect(card.style.left).toBe("406px");
    });
  });

  it("draws the spotlight ring around a targeted step", async () => {
    const { container } = render(TourOverlay);
    await startTour();

    // Step A has no target, so it dims with a plain scrim.
    expect(container.querySelector(".tour-scrim")).not.toBeNull();
    expect(container.querySelector(".tour-ring")).toBeNull();

    await fireEvent.click(screen.getByText("Next"));

    await vi.waitFor(() => {
      expect(container.querySelector(".tour-ring")).not.toBeNull();
    });
    expect(container.querySelector(".tour-scrim")).toBeNull();
    // A plain target ring does not flash; only steps that ask for it do.
    expect(container.querySelector(".tour-ring.is-pulsing")).toBeNull();

    await fireEvent.click(screen.getByText("Next"));
    await vi.waitFor(() => {
      expect(container.querySelector(".tour-ring.is-pulsing")).not.toBeNull();
    });
  });

  it("places the card below the target, or above when there is no room", async () => {
    const target = document.querySelector('[data-tour="mods-add-mod"]');
    const { container } = render(TourOverlay);
    await startTour();
    await fireEvent.click(screen.getByText("Next"));

    const card = container.querySelector(".tour-card") as HTMLElement;
    await vi.waitFor(() => {
      expect(card.style.top).toBe("154px");
    });

    // A tall target with no room below: the card flips above and clamps to the
    // viewport margin.
    target!.getBoundingClientRect = () =>
      ({
        top: 100,
        left: 100,
        width: 120,
        height: 640,
        bottom: 740,
        right: 220,
        x: 100,
        y: 100,
        toJSON: () => ({}),
      }) as DOMRect;
    window.dispatchEvent(new Event("resize"));

    await vi.waitFor(() => {
      expect(card.style.top).toBe("12px");
    });
  });

  it("steps back and forward with the arrow keys", async () => {
    render(TourOverlay);
    await startTour();

    await fireEvent.keyDown(window, { key: "ArrowRight" });
    expect(screen.getByText("Body two")).not.toBeNull();

    await fireEvent.keyDown(window, { key: "ArrowLeft" });
    expect(screen.getByText("Body one")).not.toBeNull();
  });

  it("closes on Escape", async () => {
    render(TourOverlay);
    await startTour();

    await fireEvent.keyDown(window, { key: "Escape" });

    expect(screen.queryByText("Body one")).toBeNull();
    expect(get(tourState).running).toBe(false);
  });

  it("closes for good on Skip tour and remembers it", async () => {
    render(TourOverlay);
    await startTour();

    await fireEvent.click(screen.getByText("Skip tour"));

    expect(screen.queryByText("Body one")).toBeNull();
    expect(get(tourState).running).toBe(false);
    expect(updateConfig).toHaveBeenCalledWith({ tutorial_complete: true });
  });

  it("pulses Next only while the user is meant to press it", async () => {
    render(TourOverlay);
    await startTour();

    const next = screen.getByText("Next") as HTMLButtonElement;
    expect(next.className).toContain("is-pulsing");
    expect(next.disabled).toBe(false);

    await fireEvent.click(next);
    expect(screen.getByText("Body two")).not.toBeNull();
  });

  it("holds Skip tour and Escape back while the first-run setup is on screen", async () => {
    render(TourOverlay);
    await startTour();
    expect(screen.queryByText("Skip tour")).not.toBeNull();

    // Setup comes before the app tour, so there is nothing to cancel yet.
    tourState.update((state) => ({ ...state, setupPending: true }));
    await vi.waitFor(() => {
      expect(screen.queryByText("Skip tour")).toBeNull();
    });

    await fireEvent.keyDown(window, { key: "Escape" });
    expect(get(tourState).running).toBe(true);

    // Past the setup cards it is the user's tour again.
    tourState.update((state) => ({ ...state, setupPending: false }));
    await vi.waitFor(() => {
      expect(screen.queryByText("Skip tour")).not.toBeNull();
    });
  });

  it("carries the setup fields in the card, with Set up later instead of Skip", async () => {
    render(TourOverlay);
    await startTour();
    currentStep.set({
      id: "setup",
      title: "Set up: your game folder",
      body: "Ready or Not's installation folder.",
      setup: true,
    });
    tourState.update((state) => ({ ...state, setupPending: true }));

    await vi.waitFor(() => {
      expect(screen.queryByText("Skip tour")).toBeNull();
    });
    // The form is the card's own content, and the way out is named here.
    expect(screen.getByText("Set up later")).not.toBeNull();
    expect(screen.getByText("Game path")).not.toBeNull();

    await fireEvent.click(screen.getByText("Set up later"));
    expect(updateConfig).toHaveBeenCalledWith({
      setup_wizard_complete: true,
    });
  });
});
