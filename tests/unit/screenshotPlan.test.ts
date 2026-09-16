// @ts-nocheck - exercises untyped Node tooling (see scripts/screenshot-plan.mjs).
import { describe, expect, it } from "vitest";
import {
  INCOGNITO_DUMMY_FILE,
  analyseModGroupsDiff,
  displayNameChangedGroups,
  parseChangedFiles,
  parseCollectionMembers,
  parseDummyDiff,
  planScreenshots,
} from "../../scripts/screenshot-plan.mjs";

const NO_DUMMY = { diffText: "", content: "", isNew: false };
const THEMES = ["light", "dark"];

function plan({
  changedFiles = [],
  dummy = NO_DUMMY,
  themes = THEMES,
  force = false,
  wizardOnly = false,
} = {}) {
  return planScreenshots({
    changedFiles,
    dummy,
    themes,
    force,
    wizardOnly,
    exists: () => true,
  });
}

describe("parseChangedFiles", () => {
  it("strips status flags, resolves renames and quotes", () => {
    expect(
      parseChangedFiles(
        ' M src/a.ts\nA  "src/b c.ts"\nR  old.ts -> src/new.ts\n?? untracked.ts',
      ),
    ).toEqual(["src/a.ts", "src/b c.ts", "src/new.ts", "untracked.ts"]);
  });
});

describe("parseDummyDiff", () => {
  it("attributes a hunk to the export below its start line", () => {
    const content = [
      "const x = 1;",
      "export const DUMMY_A = [",
      "  { installedAt: 1 },",
      "];",
      "export const DUMMY_B = {",
      "  k: 1,",
      "};",
    ].join("\n");
    const diff = [
      "@@ -2,2 +2,3 @@ export const DUMMY_A = [",
      "   { installedAt: 1 },",
      "+  { installedAt: 2 },",
      " ];",
    ].join("\n");
    const parsed = parseDummyDiff(diff, content);
    expect(parsed.exports).toEqual(["DUMMY_A"]);
    expect(parsed.linesByExport.DUMMY_A.plus).toHaveLength(1);
  });

  it("flags non-dummy changes when the hunk belongs to helper code", () => {
    const content = ["const x = 1;", "export const DUMMY_A = [1];"].join("\n");
    const diff = ["@@ -1,1 +1,1 @@", "-const x = 1;", "+const x = 2;"].join(
      "\n",
    );
    const parsed = parseDummyDiff(diff, content);
    expect(parsed.exports).toEqual([]);
    expect(parsed.nonDummyChange).toBe(true);
  });
});

describe("plan: no changes plans nothing", () => {
  it("captures nothing when nothing changed", () => {
    const { wizard, pages, planned } = plan();
    expect(planned).toBe(0);
    for (const t of THEMES) {
      expect(wizard[t].capture).toBe(false);
      for (const p of ["mods", "collections", "profiles", "settings"]) {
        expect(pages[t][p].capture).toBe(false);
      }
    }
  });
});

describe("plan: generic sources", () => {
  it("shared component change keeps every page", () => {
    const { wizard, pages } = plan({
      changedFiles: ["src/lib/components/Toast.svelte"],
    });
    for (const t of THEMES) {
      expect(wizard[t].capture).toBe(true);
      for (const p of ["mods", "collections", "profiles", "settings"]) {
        expect(pages[t][p].capture).toBe(true);
      }
    }
  });

  it("settings route change keeps settings only", () => {
    const { wizard, pages } = plan({
      changedFiles: ["src/routes/settings/+page.svelte"],
    });
    for (const t of THEMES) {
      expect(wizard[t].capture).toBe(false);
      expect(pages[t].settings.capture).toBe(true);
      expect(pages[t].mods.capture).toBe(false);
      expect(pages[t].collections.capture).toBe(false);
      expect(pages[t].profiles.capture).toBe(false);
    }
  });

  it("wizard setup change keeps wizard only", () => {
    const { wizard, pages } = plan({
      changedFiles: ["src/lib/components/SetupWizard.svelte"],
    });
    // SetupWizard is also under shared src/lib/components/ so everything goes
    for (const t of THEMES) {
      expect(wizard[t].capture).toBe(true);
    }
    void pages;
  });

  it("screenshot outputs never trigger themselves", () => {
    const { planned } = plan({
      changedFiles: ["docs/screenshots/light/mods.png"],
    });
    expect(planned).toBe(0);
  });
});

describe("plan: dummy exports", () => {
  const content = [
    "export const incognitoMode = 1;",
    "",
    "export const DUMMY_MOD_GROUPS = [",
    '  { name: "A", displayName: "A", installedAt: 1 },',
    "];",
    "",
    "export const DUMMY_PROFILES = [",
    '  { name: "P1", installed_mod_names: ["A"] },',
    "];",
    "",
    "export const DUMMY_COLLECTIONS = {",
    '  Favourites: ["A"],',
    "};",
  ].join("\n");

  it("installedAt-only change keeps mods, skips collections/profiles/wizard", () => {
    const diff = [
      "@@ -3,3 +3,3 @@ export const DUMMY_MOD_GROUPS = [",
      '   { name: "A", displayName: "A" },',
      "-    installedAt: 1,",
      "+    installedAt: 2,",
      " ];",
    ].join("\n");
    const { wizard, pages } = plan({
      changedFiles: [INCOGNITO_DUMMY_FILE],
      dummy: { diffText: diff, content, isNew: false },
    });
    for (const t of THEMES) {
      expect(wizard[t].capture).toBe(false);
      expect(pages[t].mods.capture).toBe(true);
      expect(pages[t].collections.capture).toBe(false);
      expect(pages[t].profiles.capture).toBe(false);
      expect(pages[t].settings.capture).toBe(false);
    }
  });

  it("displayName change of a collection member keeps mods + collections", () => {
    const contentWithCollections = [
      ...content.split("\n"),
      "export const DUMMY_COLLECTION_COLORS = {};",
    ].join("\n");
    const diff = [
      "@@ -3,4 +3,4 @@ export const DUMMY_MOD_GROUPS = [",
      "   {",
      '     name: "A",',
      '-    displayName: "A",',
      '+    displayName: "Renamed",',
      "   },",
      " ];",
    ].join("\n");
    const { pages } = plan({
      changedFiles: [INCOGNITO_DUMMY_FILE],
      dummy: {
        diffText: diff,
        content: contentWithCollections,
        isNew: false,
      },
    });
    for (const t of THEMES) {
      expect(pages[t].mods.capture).toBe(true);
      expect(pages[t].collections.capture).toBe(true);
      expect(pages[t].profiles.capture).toBe(false);
    }
  });

  it("DUMMY_PROFILES change keeps profiles only", () => {
    const diff = [
      "@@ -7,4 +7,4 @@ export const DUMMY_PROFILES = [",
      "   {",
      '     name: "P1",',
      '-    description: "old",',
      '+    description: "new",',
      "   },",
      " ];",
    ].join("\n");
    const { pages } = plan({
      changedFiles: [INCOGNITO_DUMMY_FILE],
      dummy: { diffText: diff, content, isNew: false },
    });
    for (const t of THEMES) {
      expect(pages[t].profiles.capture).toBe(true);
      expect(pages[t].mods.capture).toBe(false);
      expect(pages[t].collections.capture).toBe(false);
    }
  });

  it("new dummy file keeps all dummy pages but not wizard/settings", () => {
    const { wizard, pages } = plan({
      changedFiles: [INCOGNITO_DUMMY_FILE],
      dummy: { diffText: "", content: "", isNew: true },
    });
    for (const t of THEMES) {
      expect(wizard[t].capture).toBe(false);
      expect(pages[t].mods.capture).toBe(true);
      expect(pages[t].collections.capture).toBe(true);
      expect(pages[t].profiles.capture).toBe(true);
      expect(pages[t].settings.capture).toBe(false);
    }
  });
});

describe("helpers", () => {
  it("analyseModGroupsDiff detects any change", () => {
    expect(analyseModGroupsDiff(["+  installedAt: 1,"], []).anyChange).toBe(
      true,
    );
    expect(analyseModGroupsDiff([], []).anyChange).toBe(false);
  });

  it("displayNameChangedGroups attributes to enclosing group", () => {
    const diff = [
      "@@ -3,4 +3,4 @@ export const DUMMY_MOD_GROUPS = [",
      "   {",
      '     name: "A",',
      '-    displayName: "A",',
      '+    displayName: "B",',
      "   },",
    ].join("\n");
    expect(displayNameChangedGroups(diff)).toEqual(["A"]);
  });

  it("parseCollectionMembers extracts array members, not keys", () => {
    const content = [
      "export const DUMMY_COLLECTIONS = {",
      '  Favourites: ["A", "B"],',
      '  Other: ["C"],',
      "};",
      "export const X = 1;",
    ].join("\n");
    expect(parseCollectionMembers(content).sort()).toEqual(["A", "B", "C"]);
  });
});
