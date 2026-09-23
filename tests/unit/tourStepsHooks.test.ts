import { get } from "svelte/store";
import { describe, expect, it, vi, type Mock } from "vitest";
import { TOUR_STEPS } from "../../src/lib/tour/steps";
import { setupWizardPage } from "../../src/lib/stores/setupWizard";
import { addModTourCommand } from "../../src/lib/stores/tourUi";
import type {
  TourActionName,
  TourContext,
  TourRoute,
} from "../../src/lib/tour/types";

type WaitFor = <T>(
  predicate: () => T | null | false | undefined,
  timeoutMs: number,
) => Promise<T | null>;

function fakeCtx() {
  return {
    call: vi.fn<(name: TourActionName) => Promise<unknown>>(async () => {}),
    goto: vi.fn<(route: TourRoute) => Promise<void>>(async () => {}),
    waitFor: vi.fn<WaitFor>(async () => null),
    waitForSelector: vi.fn<
      (selector: string, timeoutMs?: number) => Promise<Element | null>
    >(async () => null),
  } as unknown as TourContext & {
    call: Mock<(name: TourActionName) => Promise<unknown>>;
    waitForSelector: Mock<
      (selector: string, timeoutMs?: number) => Promise<Element | null>
    >;
  };
}

function stepById(id: string) {
  const step = TOUR_STEPS.find((entry) => entry.id === id);
  if (!step) throw new Error(`missing tour step: ${id}`);
  return step;
}

describe("tour step hooks", () => {
  it("runs every step's hooks without throwing", async () => {
    for (const step of TOUR_STEPS) {
      const ctx = fakeCtx();
      await step.enter?.(ctx);
      // Steps without a ready hook are always ready: assert the resolved
      // value unconditionally rather than branching around the expect.
      expect((await step.ready?.(ctx)) ?? false).toBe(false);
      await step.leave?.(ctx);
    }
  });

  it("switches the Add Mod dialog to the link tab and prefills the example", async () => {
    const ctx = fakeCtx();
    await stepById("add-mod-link").enter?.(ctx);
    expect(get(addModTourCommand)).toEqual({
      tab: "link",
      link: "https://mod.io/g/readyornot/m/uon-official#description",
    });
  });

  it("reopens the Add Mod dialog when a Back re-enters the link step", async () => {
    document.body.innerHTML = "";
    const ctx = fakeCtx();
    await stepById("add-mod-link").enter?.(ctx);
    expect(ctx.call).toHaveBeenCalledWith("mods:open-add-mod");
  });

  it("skips reopening when the dialog is already open", async () => {
    document.body.innerHTML = '<div data-tour="addmod-panel"></div>';
    const ctx = fakeCtx();
    await stepById("add-mod-link").enter?.(ctx);
    expect(ctx.call).not.toHaveBeenCalled();
    document.body.innerHTML = "";
  });

  it("switches to the Local File tab and back again", async () => {
    await stepById("add-mod-file").enter?.(fakeCtx());
    expect(get(addModTourCommand)).toEqual({ tab: "file" });

    await stepById("add-mod-submit").enter?.(fakeCtx());
    expect(get(addModTourCommand)).toEqual({ tab: "link" });
  });

  it("closes the Add Mod dialog and the export dialog when leaving their steps", async () => {
    const submitCtx = fakeCtx();
    await stepById("add-mod-submit").leave?.(submitCtx);
    expect(submitCtx.call).toHaveBeenCalledWith("mods:close-add-mod");

    const exportCtx = fakeCtx();
    await stepById("settings-export-dialog").leave?.(exportCtx);
    expect(exportCtx.call).toHaveBeenCalledWith("settings:close-export-modal");

    const profileCtx = fakeCtx();
    await stepById("profile-actions").leave?.(profileCtx);
    expect(profileCtx.call).toHaveBeenCalledWith("profiles:close-create-form");

    const logCtx = fakeCtx();
    await stepById("import-log-close").leave?.(logCtx);
    expect(logCtx.call).toHaveBeenCalledWith("shell:close-import-log");

    const credsCtx = fakeCtx();
    await stepById("settings-sync-credentials").leave?.(credsCtx);
    expect(credsCtx.call).toHaveBeenCalledWith(
      "settings:close-sync-credentials",
    );

    // Leaving the theme card puts the window back and drops the demo timer.
    const themeCtx = fakeCtx();
    await stepById("settings-theme").leave?.(themeCtx);
    expect(themeCtx.call).toHaveBeenCalledWith("settings:stop-theme-demo");
  });

  it("opens the credentials dialog and shows the theme change", async () => {
    const credsCtx = fakeCtx();
    await stepById("settings-sync-credentials").enter?.(credsCtx);
    expect(credsCtx.call).toHaveBeenCalledWith(
      "settings:open-sync-credentials",
    );

    const themeCtx = fakeCtx();
    await stepById("settings-theme").enter?.(themeCtx);
    expect(themeCtx.call).toHaveBeenCalledWith("settings:demo-theme");
  });

  it("puts the app back into shape when a step is re-entered with Back", async () => {
    // Back must undo whatever the step ahead of it tidied away, or the card
    // describes controls that are not on screen any more.
    document.body.innerHTML = "";
    const expectations: Array<[string, string]> = [
      ["add-mod", "mods:close-add-mod"],
      ["add-mod-url", "mods:open-add-mod"],
      ["add-mod-file", "mods:open-add-mod"],
      ["add-mod-submit", "mods:open-add-mod"],
      ["import-log", "shell:open-import-log"],
      ["import-log-close", "shell:open-import-log"],
      ["profiles-create", "profiles:close-create-form"],
      ["profile-name", "profiles:ensure-create-form"],
      ["profile-description", "profiles:ensure-create-form"],
      ["profile-actions", "profiles:ensure-create-form"],
      ["profile-apply", "profiles:close-create-form"],
      ["settings-export-dialog", "settings:ensure-export-modal"],
    ];

    for (const [id, action] of expectations) {
      const ctx = fakeCtx();
      await stepById(id).enter?.(ctx);
      expect(ctx.call).toHaveBeenCalledWith(action);
    }
  });

  it("returns to Settings with Back from the closing card", () => {
    expect(stepById("settings-tutorial").route).toBe("/settings");
  });

  it("resolves the mod row through the first-row helper", () => {
    document.body.innerHTML = '<li data-tour="mod-row"></li>';
    const step = stepById("mod-row");
    expect(step.resolveTarget?.()).not.toBeNull();
    document.body.innerHTML = "";
    expect(step.resolveTarget?.()).toBeNull();
  });

  it("waits for the export dialog before enabling Next", async () => {
    const ctx = fakeCtx();
    await stepById("settings-export-modal").ready?.(ctx);
    expect(ctx.waitForSelector).toHaveBeenCalledWith(
      '[data-tour="export-panel"]',
      60000,
    );
  });

  it("shows the setup page each setup card is about", async () => {
    const pages: Array<[string, number]> = [
      ["setup-game-path", 1],
      ["setup-modio", 2],
      ["setup-nexus", 3],
      ["setup-modpack", 4],
    ];
    for (const [id, page] of pages) {
      await stepById(id).enter?.(fakeCtx());
      expect(get(setupWizardPage)).toBe(page);
    }
  });
});
