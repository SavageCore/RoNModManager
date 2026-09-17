import { goto } from "$app/navigation";
import { page } from "$app/stores";
import { get, writable } from "svelte/store";
import { tick } from "svelte";
import { updateConfig } from "$lib/api/commands";
import { callTourAction } from "./registry";
import { TOUR_STEPS } from "./steps";
import type { TourContext, TourRoute, TourStep } from "./types";

// Back re-entry rules:
// 1. enter() only ever moves the app forward into the state a step needs and is
//    idempotent, so Back re-enters a step instead of restoring a snapshot.
// 2. leave() closes what a step opened and is safe to call twice.
// 3. Back never undoes real work (the example mod install is kept).

const TARGET_TIMEOUT_MS = 3000;
const POLL_MS = 150;

export type TourState = {
  running: boolean;
  index: number;
  total: number;
  rect: DOMRect | null;
  showRing: boolean;
  waitingForTarget: boolean;
  ready: boolean;
  readyTimedOut: boolean;
  busy: boolean;
  /// The first-run setup cards are on screen. Skip tour and Escape are held
  /// back until the user is past them.
  setupPending: boolean;
};

const IDLE: TourState = {
  running: false,
  index: 0,
  total: 0,
  rect: null,
  showRing: true,
  waitingForTarget: false,
  ready: true,
  readyTimedOut: false,
  busy: false,
  setupPending: false,
};

export const tourState = writable<TourState>(IDLE);
export const currentStep = writable<TourStep | null>(null);

/// The steps of the run in progress. A replay drops the setup cards, which
/// only a first launch needs.
let steps: TourStep[] = TOUR_STEPS;
/// Index of the last setup card in this run, or -1 when the run has none.
let setupEndIndex = -1;
let current: TourStep | null = null;
let targetEl: Element | null = null;
let rectTimer: ReturnType<typeof setInterval> | null = null;
let stepTimer: ReturnType<typeof setTimeout> | null = null;
let enterToken = 0;

function selectSteps(setup: boolean): TourStep[] {
  return setup ? TOUR_STEPS : TOUR_STEPS.filter((step) => !step.setup);
}

function setupPendingAt(index: number): boolean {
  return setupEndIndex >= 0 && index <= setupEndIndex;
}

function waitFor<T>(
  predicate: () => T | null | false | undefined,
  timeoutMs: number,
): Promise<T | null> {
  return new Promise((resolve) => {
    const deadline = Date.now() + timeoutMs;
    const done = (): boolean => {
      const value = predicate();
      if (value) {
        resolve(value as T);
        return true;
      }
      if (Date.now() > deadline) {
        resolve(null);
        return true;
      }
      return false;
    };
    if (done()) return;
    const timer = setInterval(() => {
      if (done()) clearInterval(timer);
    }, POLL_MS);
  });
}

function nextFrame(): Promise<void> {
  // requestAnimationFrame is missing in some headless webviews and jsdom.
  return new Promise((resolve) => {
    if (typeof requestAnimationFrame === "function") {
      requestAnimationFrame(() => resolve());
    } else {
      setTimeout(resolve, 0);
    }
  });
}

const ctx: TourContext = {
  call: (name) => callTourAction(name),
  goto: async (route: TourRoute) => {
    if (get(page).url.pathname !== route) {
      await goto(route);
      await nextFrame();
      await tick();
    }
  },
  waitFor,
  waitForSelector: (selector, timeoutMs = TARGET_TIMEOUT_MS) =>
    waitFor(() => document.querySelector(selector), timeoutMs),
};

function sameRect(a: DOMRect | null, b: DOMRect | null): boolean {
  if (!a || !b) return a === b;
  return (
    a.top === b.top &&
    a.left === b.left &&
    a.width === b.width &&
    a.height === b.height
  );
}

function measure(force = false): void {
  const next =
    targetEl && targetEl.isConnected ? targetEl.getBoundingClientRect() : null;
  tourState.update((state) =>
    force || !sameRect(state.rect, next) ? { ...state, rect: next } : state,
  );
}

function handleGeometryChange(): void {
  measure(true);
}

function startRectLoop(): void {
  stopRectLoop();
  rectTimer = setInterval(() => measure(), 200);
  window.addEventListener("resize", handleGeometryChange);
  document.addEventListener("scroll", handleGeometryChange, true);
}

function stopRectLoop(): void {
  if (rectTimer) clearInterval(rectTimer);
  rectTimer = null;
  window.removeEventListener("resize", handleGeometryChange);
  document.removeEventListener("scroll", handleGeometryChange, true);
}

async function resolveTarget(step: TourStep): Promise<Element | null> {
  const timeout = step.targetTimeoutMs ?? TARGET_TIMEOUT_MS;
  if (step.resolveTarget) return await waitFor(step.resolveTarget, timeout);
  if (step.target) return await ctx.waitForSelector(step.target, timeout);
  if (step.anchorTo) return await ctx.waitForSelector(step.anchorTo, timeout);
  return null;
}

function hasTarget(step: TourStep): boolean {
  return Boolean(step.target || step.resolveTarget || step.anchorTo);
}

function clearStepTimer(): void {
  if (stepTimer) clearTimeout(stepTimer);
  stepTimer = null;
}

/// Auto-advance can be asked for before the step has finished setting itself
/// up (a `ready` that resolves immediately). Retry until the engine is free
/// rather than dropping the advance on the floor.
function autoAdvance(): void {
  const attempt = (tries: number): void => {
    const state = get(tourState);
    if (!state.running) return;
    if (!state.busy) {
      void advance(false);
      return;
    }
    if (tries >= 40) return;
    setTimeout(() => attempt(tries + 1), POLL_MS);
  };
  attempt(0);
}

async function enter(step: TourStep, allowAutoAdvance = true): Promise<void> {
  enterToken += 1;
  const token = enterToken;
  current = step;
  currentStep.set(step);
  clearStepTimer();
  tourState.update((state) => ({
    ...state,
    busy: true,
    // A step with a nextAction never disables Next: pressing it does the thing.
    ready: !step.ready || Boolean(step.nextAction),
    readyTimedOut: false,
    showRing: step.showRing !== false && hasTarget(step),
    waitingForTarget: hasTarget(step),
  }));

  try {
    if (step.route) await ctx.goto(step.route);
    if (step.enter) await step.enter(ctx);

    targetEl = await resolveTarget(step);
    if (typeof targetEl?.scrollIntoView === "function") {
      targetEl.scrollIntoView({ block: "center", behavior: "instant" });
    }
    await tick();
    measure(true);
  } catch (error) {
    // One bad step must not wedge the tour: without this the engine would stay
    // busy and refuse every Next and Back from here on.
    console.error(`Tour step "${step.id}" failed to set up:`, error);
  } finally {
    tourState.update((state) => ({
      ...state,
      busy: false,
      waitingForTarget: hasTarget(step) && !targetEl,
    }));
    startRectLoop();
  }

  if (step.ready) {
    void step.ready(ctx).then((ok) => {
      if (token !== enterToken) return;
      tourState.update((state) => ({
        ...state,
        ready: true,
        readyTimedOut: !ok,
      }));
      if (ok && step.advanceOnReady && allowAutoAdvance) autoAdvance();
    });
  }

  // The step's own pacing, so it re-arms even when the user steps back into it
  // - otherwise a step with nothing to wait for leaves them stuck.
  if (step.autoAdvanceMs) {
    stepTimer = setTimeout(() => {
      stepTimer = null;
      void advance(false);
    }, step.autoAdvanceMs);
  }
}

export async function startTour(
  options: { setup?: boolean } = {},
): Promise<void> {
  steps = selectSteps(options.setup === true);
  setupEndIndex = steps.findLastIndex((step) => step.setup === true);
  tourState.set({
    ...IDLE,
    running: true,
    total: steps.length,
    setupPending: setupPendingAt(0),
  });
  await enter(steps[0]);
}

async function leaveCurrent(): Promise<void> {
  clearStepTimer();
  if (current?.leave) await current.leave(ctx);
  targetEl = null;
  stopRectLoop();
}

export async function nextStep(): Promise<void> {
  await advance(true);
}

/// `runNextAction` is false for the engine's own auto-advances: when the user
/// clicked the control themselves, Next must not do it a second time.
async function advance(runNextAction: boolean): Promise<void> {
  const state = get(tourState);
  if (!state.running || state.busy) return;
  const action = current?.nextAction;
  if (runNextAction && action) {
    try {
      // False means the control refused (a form that failed validation), so
      // the card stays put with the app's own error on screen beside it.
      if ((await ctx.call(action)) === false) return;
    } catch (error) {
      console.error(`Tour action "${action}" failed:`, error);
    }
  }
  await leaveCurrent();
  const index = state.index + 1;
  if (index >= steps.length) {
    // The last card navigates on entry, so finishing just closes the tour.
    await closeTour();
    return;
  }
  tourState.update((s) => ({
    ...s,
    index,
    setupPending: setupPendingAt(index),
  }));
  await enter(steps[index]);
}

export async function prevStep(): Promise<void> {
  const state = get(tourState);
  if (!state.running || state.busy || state.index === 0) return;
  await leaveCurrent();
  const index = state.index - 1;
  tourState.update((s) => ({
    ...s,
    index,
    setupPending: setupPendingAt(index),
  }));
  // Going back means the user wants to re-read the step, so a step that
  // normally advances on its own stays put this time.
  await enter(steps[index], false);
}

/// Drops the setup cards from the run in progress, for a user who deferred
/// setup: the tour carries on where those cards would have left it, and can be
/// cancelled from there.
export async function skipSetupSteps(): Promise<void> {
  const state = get(tourState);
  if (!state.running || state.busy) return;
  if (setupEndIndex < 0 || state.index > setupEndIndex) return;
  await leaveCurrent();
  const index = setupEndIndex + 1;
  if (index >= steps.length) {
    await closeTour();
    return;
  }
  tourState.update((s) => ({
    ...s,
    index,
    setupPending: setupPendingAt(index),
  }));
  await enter(steps[index]);
}

/// The user's way out of the first-run setup: the surface that owns setup
/// marks it settled, so the app stops asking, and the setup cards come off
/// this run. The tour's own Skip is held back until then.
export async function deferSetup(): Promise<void> {
  await ctx.call("setup:defer");
  await skipSetupSteps();
}

export async function closeTour(): Promise<void> {
  // The first-run setup cards hold the user until setup is done or deferred:
  // Skip tour, Escape and anything else that closes the tour land here.
  const state = get(tourState);
  if (state.running && state.setupPending) return;
  const step = current;
  clearStepTimer();
  if (step?.leave) await step.leave(ctx);
  current = null;
  currentStep.set(null);
  targetEl = null;
  stopRectLoop();
  enterToken += 1;
  tourState.set(IDLE);
  await updateConfig({ tutorial_complete: true }).catch(() => {});
}

export async function clearTutorialComplete(): Promise<void> {
  await updateConfig({ tutorial_complete: false }).catch(() => {});
}
