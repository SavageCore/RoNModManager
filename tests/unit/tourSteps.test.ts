import { describe, expect, it } from "vitest";
import { TOUR_STEPS } from "../../src/lib/tour/steps";

function stepById(id: string) {
  const step = TOUR_STEPS.find((entry) => entry.id === id);
  if (!step) throw new Error(`missing tour step: ${id}`);
  return step;
}

describe("TOUR_STEPS", () => {
  it("has unique step ids", () => {
    const ids = TOUR_STEPS.map((step) => step.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("targets only data-tour anchors", () => {
    for (const step of TOUR_STEPS) {
      if (step.target) expect(step.target).toMatch(/^\[data-tour="/);
    }
  });

  it("routes only to known pages", () => {
    const routes = new Set(
      TOUR_STEPS.map((step) => step.route).filter(Boolean),
    );
    for (const route of routes) {
      expect(["/mods", "/profiles", "/settings"]).toContain(route);
    }
  });

  it("waits long enough for the example download to land", () => {
    expect(stepById("mod-row").targetTimeoutMs ?? 0).toBeGreaterThanOrEqual(
      60000,
    );
  });

  it("keeps the mod.io example link in the copy", () => {
    expect(stepById("add-mod-url").body).toContain("uon-official");
  });

  it("does not claim a download is still running once the row exists", () => {
    for (const step of TOUR_STEPS) {
      expect(step.body ?? "").not.toMatch(/waiting for the download/i);
    }
  });

  it("explains the row switch and the delete button on their own controls", () => {
    const toggle = stepById("mod-row-toggle");
    expect(toggle.target).toBe('[data-tour="mod-row-toggle"]');
    expect(toggle.body).toContain("active profile");

    const remove = stepById("mod-row-delete");
    expect(remove.target).toBe('[data-tour="mod-row-delete"]');
    expect(remove.body).toContain("removes the mod");
  });

  it("keeps the context menu step to short bullets instead of prose", () => {
    const menuStep = stepById("mod-row-menu");
    expect(menuStep.body?.length ?? 0).toBeLessThan(120);
    expect(menuStep.bullets?.length ?? 0).toBeGreaterThanOrEqual(5);

    const bullets = (menuStep.bullets ?? []).join(" ");
    for (const topic of [
      "Refresh metadata",
      "Manage tags",
      "Manage collections",
      "Manage add-ons",
      "Mark as broken",
      "Edit link",
    ]) {
      expect(bullets).toContain(topic);
    }
    expect(bullets).toContain("Visuals");
    expect(bullets).toContain("LSPD");
  });

  it("keeps every card body short", () => {
    for (const step of TOUR_STEPS) {
      expect(step.body?.length ?? 0).toBeLessThan(300);
      for (const bullet of step.bullets ?? []) {
        expect(bullet.length).toBeLessThan(180);
      }
    }
  });

  it("moves the profile form cards aside so the form stays usable", () => {
    for (const id of [
      "profile-name",
      "profile-description",
      "profile-actions",
    ]) {
      expect(stepById(id).placement).toBe("left");
    }
    // Save and Cancel are ringed as one wide pill.
    expect(stepById("profile-actions").target).toBe(
      '[data-tour="profile-actions"]',
    );
  });

  it("advances by itself once the user has done the gated action", () => {
    // Pressing Next again after the click is what confused people.
    for (const id of [
      "add-mod",
      "add-mod-submit",
      "import-log-close",
      "profiles-create",
      "profile-actions",
      "settings-export-modal",
    ]) {
      expect(stepById(id).advanceOnReady).toBe(true);
    }
  });

  it("flashes the ring on pressable controls instead of a steady border", () => {
    const flashed = TOUR_STEPS.filter((step) => step.pulseTarget).map(
      (step) => step.id,
    );
    expect(flashed).toEqual([
      "add-mod",
      "add-mod-link",
      "add-mod-submit",
      "import-log-close",
      "mod-row-toggle",
      "mod-row-delete",
      "header-profile",
      "profiles-create",
      "profile-actions",
      "profile-apply",
      "header-refresh",
      "settings-export-modal",
      "settings-tutorial",
    ]);

    // Things the user only reads keep the steady ring: rows, panels, fields,
    // the sidebar items the tour navigates for them, and Sync Now (something
    // the user is not expected to press during the tour).
    for (const id of [
      "mod-row",
      "mod-row-menu",
      "import-log",
      "nav-profiles",
      "nav-settings",
      "profile-name",
      "profile-description",
      "add-mod-url",
      "add-mod-file",
      "settings-theme",
      "settings-link-on-launch",
      "settings-export-dialog",
      "settings-sync-now",
    ]) {
      expect(stepById(id).pulseTarget).toBeUndefined();
    }
  });

  it("does the highlighted control's job from Next", () => {
    const stepped = {
      "setup-game-path": "setup:save-game-path",
      "setup-modio": "setup:save-modio",
      "setup-nexus": "setup:save-nexus",
      "setup-modpack": "setup:finish",
      "add-mod": "mods:open-add-mod",
      "add-mod-submit": "mods:submit-add-mod",
      "import-log-close": "shell:close-import-log",
      "profiles-create": "profiles:open-create-form",
      "profile-actions": "profiles:submit-create-form",
      "settings-export-modal": "settings:open-export-modal",
    };
    for (const [id, action] of Object.entries(stepped)) {
      expect(stepById(id).nextAction).toBe(action);
    }

    // Nothing else may quietly act on the user's behalf.
    const acting = TOUR_STEPS.filter((step) => step.nextAction).map(
      (step) => step.id,
    );
    expect(acting).toEqual(Object.keys(stepped));
  });

  it("runs the first-launch setup as the tour's opening cards", () => {
    const ids = TOUR_STEPS.map((step) => step.id);
    expect(ids.slice(0, 6)).toEqual([
      "welcome",
      "setup-game-path",
      "setup-modio",
      "setup-nexus",
      "setup-modpack",
      "add-mod",
    ]);

    // Only those four cards are setup: they are dropped from a replay, and the
    // tour cannot be cancelled while they are on screen.
    const setupIds = TOUR_STEPS.filter((step) => step.setup).map(
      (step) => step.id,
    );
    expect(setupIds).toEqual([
      "setup-game-path",
      "setup-modio",
      "setup-nexus",
      "setup-modpack",
    ]);
  });

  it("puts the setup fields in the tour card, not a second dialog", () => {
    for (const step of TOUR_STEPS.filter((entry) => entry.setup)) {
      // Nothing to ring and nothing to place beside: the card itself is the
      // surface the fields live in.
      expect(step.target).toBeUndefined();
      expect(step.route).toBe("/mods");
      expect(step.placement).toBeUndefined();
      // A steady card: the card asks for a form to be filled in, not for one
      // control to be pressed (the tour's own Next does that).
      expect(step.pulseTarget).toBeUndefined();
    }
  });

  it("highlights the sidebar item before navigating to another page", () => {
    for (const [id, anchor, route] of [
      ["nav-profiles", "nav-profiles", "/profiles"],
      ["nav-settings", "nav-settings", "/settings"],
    ]) {
      const step = stepById(id);
      expect(step.target).toBe(`[data-tour="${anchor}"]`);
      // Long enough not to rush the user off the sidebar item.
      expect(step.autoAdvanceMs ?? 0).toBeGreaterThanOrEqual(20000);
      // The page itself is opened by the step after the sidebar highlight.
      const next = TOUR_STEPS[TOUR_STEPS.indexOf(step) + 1];
      expect(next.route).toBe(route);
    }
  });

  it("keeps the export dialog step clickable instead of gated", () => {
    const step = stepById("settings-export-dialog");
    // Nothing to wait for: Next closes the dialog, so it must never be disabled.
    expect(step.ready).toBeUndefined();
    // The whole modal, so the Hosting guide link is inside the highlight too.
    expect(step.target).toBe('[data-tour="export-modal-panel"]');
  });

  it("waits for the user on the theme card instead of moving on", () => {
    const step = stepById("settings-theme");
    // The demo reverts on its own and the flashing Next is the cue: a slow
    // reader must not be pushed off the card.
    expect(step.autoAdvanceMs).toBeUndefined();
    expect(step.enter).toBeDefined();
    expect(step.leave).toBeDefined();
  });

  it("returns to the page its target lives on when stepping back", () => {
    // Without a route, a step entered with Back describes controls that are not
    // on the current page, and the card is stranded with no highlight.
    const pageBound: Record<string, string> = {
      "add-mod-link": "/mods",
      "add-mod-url": "/mods",
      "add-mod-file": "/mods",
      "add-mod-submit": "/mods",
      "import-log": "/mods",
      "import-log-close": "/mods",
      "mod-row": "/mods",
      "mod-row-toggle": "/mods",
      "mod-row-delete": "/mods",
      "mod-row-menu": "/mods",
      "header-refresh": "/mods",
      "profiles-create": "/profiles",
      "profile-name": "/profiles",
      "profile-description": "/profiles",
      "profile-actions": "/profiles",
      "profile-apply": "/profiles",
      "settings-theme": "/settings",
      "settings-export-dialog": "/settings",
      "settings-sync-credentials": "/settings",
      "settings-sync-now": "/settings",
      "settings-tutorial": "/settings",
    };
    for (const [id, route] of Object.entries(pageBound)) {
      expect(stepById(id).route).toBe(route);
    }

    // Only the controls that exist on every page may skip the route.
    const globalTargets = ["header-profile", "nav-profiles", "nav-settings"];
    for (const step of TOUR_STEPS) {
      if (!step.target || globalTargets.includes(step.id)) continue;
      expect(
        step.route,
        `${step.id} needs a route so Back can find its target`,
      ).toBeDefined();
    }
  });

  it("walks the credentials dialog before Sync Now", () => {
    const ids = TOUR_STEPS.map((step) => step.id);
    const syncNow = ids.indexOf("settings-sync-now");
    expect(ids[syncNow - 1]).toBe("settings-sync-credentials");
    expect(ids[syncNow + 1]).toBe("settings-tutorial");
    expect(stepById("settings-sync-credentials").target).toBe(
      '[data-tour="sync-auth-panel"]',
    );
  });

  it("walks the import log after the example install", () => {
    const ids = TOUR_STEPS.map((step) => step.id);
    const submit = ids.indexOf("add-mod-submit");
    expect(ids[submit + 1]).toBe("import-log");
    expect(ids[submit + 2]).toBe("import-log-close");
    expect(stepById("import-log").target).toContain("import-log-entry");
    expect(stepById("import-log-close").target).toContain("import-log-close");
    expect(stepById("import-log-close").advanceOnReady).toBe(true);
  });

  it("does not mention metadata the app never shows", () => {
    for (const step of TOUR_STEPS) {
      expect(step.body ?? "").not.toMatch(/hidden or removed/i);
    }
  });

  it("lands on the Mods page for the final card, whatever the user does", () => {
    // Replaying from Settings has to start on Mods, and the closing card is
    // read on Mods, so Finish has nothing left to navigate.
    expect(stepById("welcome").route).toBe("/mods");
    expect(TOUR_STEPS.at(-1)?.route).toBe("/mods");
    expect(TOUR_STEPS.at(-1)?.body).toContain("Press Finish");
  });

  it("never drives the tag, collection or add-on editors", () => {
    // Those editors only exist on the native right-click menu, which no DOM
    // element can target, so the tour explains them instead of opening them.
    for (const step of TOUR_STEPS) {
      const enter = step.enter?.toString() ?? "";
      expect(enter).not.toMatch(
        /open-tag-picker|open-collection-picker|open-addons-modal/,
      );
    }
  });

  it("starts with a centred welcome and ends with a centred summary", () => {
    expect(TOUR_STEPS[0].target).toBeUndefined();
    expect(TOUR_STEPS.at(-1)?.target).toBeUndefined();
  });
});
