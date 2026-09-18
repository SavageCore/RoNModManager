#!/usr/bin/env node
// @ts-nocheck - Node tooling script with JSDoc-free dynamic shapes;
// type-checked implicitly via tests/unit/screenshotPlan.test.ts instead.
/**
 * Code-based screenshot planner.
 *
 * Decides which screenshots need (re)taking purely from uncommitted source
 * changes - there is deliberately NO pixel/RMSE image comparison anywhere
 * in this flow. The plan is computed BEFORE any app is launched (wizard
 * included), so unneeded captures are skipped entirely instead of being
 * taken and discarded.
 *
 * How it works:
 * - Each page declares which source files can affect it (PAGE_SOURCES plus
 *   SHARED_SOURCES, matched by path prefix against `git status` output).
 * - src/lib/stores/incognitoMode.ts is special: it only feeds dummy data to
 *   the pages that render it, so the diff is parsed to find which DUMMY_*
 *   export changed, and only the consumers of that export are kept.
 * - DUMMY_MOD_GROUPS is analysed one level deeper, because collections only
 *   renders `name`/`displayName` of collection members while mods renders
 *   every field: an `installedAt`-only change keeps mods but skips
 *   collections and profiles.
 */
import { execSync } from "child_process";
import path from "path";

export const PAGES = ["wizard", "mods", "collections", "profiles", "settings"];
export const MAIN_PAGES = ["mods", "collections", "profiles", "settings"];

export const INCOGNITO_DUMMY_FILE = "src/lib/stores/incognitoMode.ts";

// Docs-site sync: `make screenshots` also refreshes the RoNModManager-site
// checkout (a sibling directory by default, overridable via SITE_REPO_DIR).
// Only mods-dark.png is synced - it is the one screenshot the site imports
// (src/pages/index.astro). Per the site's ASSETS.md, site copies are the
// 1280x840 app window, so the 40px README border is shaved off first.
export const SITE_REPO_DIR_NAME = "RoNModManager-site";
export const SITE_SCREENSHOT_BORDER = 40;
export const SITE_SCREENSHOT_SYNC = [
  { source: "dark/mods.png", dest: "src/assets/screenshots/mods-dark.png" },
];

/** Site checkout dir: SITE_REPO_DIR, or the sibling RoNModManager-site. */
export function siteRepoDir(rootDir, env = process.env) {
  if (env.SITE_REPO_DIR) return env.SITE_REPO_DIR;
  return path.resolve(rootDir, "..", SITE_REPO_DIR_NAME);
}

// Source files whose uncommitted changes affect every page (shared chrome,
// theming, global styles, layout). incognitoMode.ts is deliberately NOT
// here - it is mapped explicitly via DUMMY_EXPORT_CONSUMERS below.
export const SHARED_SOURCES = [
  "src/lib/components/",
  "src/lib/stores/",
  "src/lib/theme.ts",
  "src/lib/utils/",
  "src/routes/+layout.svelte",
  "src/app.css",
];

// Page-specific sources, in addition to SHARED_SOURCES. Paths are matched
// by prefix against repo-relative changed files.
export const PAGE_SOURCES = {
  // The setup wizard is the guided tour's opening cards now, so the tour's
  // step copy, engine and overlay all feed the wizard shot.
  wizard: ["src/lib/components/SetupWizard.svelte", "src/lib/tour/"],
  mods: ["src/routes/mods/"],
  collections: ["src/routes/collections/"],
  profiles: ["src/routes/profiles/"],
  settings: ["src/routes/settings/"],
};

// Which pages render each DUMMY_* export. DUMMY_MOD_GROUPS additionally
// affects collections, but only when name/displayName of a collection
// member changed (see analyseModGroupsDiff) - installedAt-only changes
// are mods-only.
export const DUMMY_EXPORT_CONSUMERS = {
  DUMMY_MOD_GROUPS: ["mods"],
  DUMMY_PROFILE_MODS: ["mods"],
  DUMMY_COLLECTIONS: ["mods", "collections"],
  DUMMY_COLLECTION_COLORS: ["mods", "collections"],
  DUMMY_TAGS: ["mods"],
  DUMMY_PROFILES: ["profiles"],
  DUMMY_MOD_UPDATES: ["mods"],
};

const ALL_DUMMY_PAGES = ["mods", "collections", "profiles"];

// Group-level (4-space indent) field lines inside DUMMY_MOD_GROUPS.
// Deeper-indented lines belong to files[]/addonFiles[] entries, not groups.
const GROUP_NAME_RE = /^[-+ ]\s{4}name:\s*"([^"]+)"/;
const GROUP_FIELD_RE = /^[+-]\s{4}(displayName|installedAt)\s*:/;
const GROUP_OTHER_FIELD_RE =
  /^[+-]\s{4}(name|installedVersion|totalSize|files|addonFiles|sourceUrl|managedByManifest)\s*:/;

/**
 * Parse `git status --porcelain -uall` output into repo-relative paths.
 * Handles rename entries ("old -> new") and quoted paths.
 */
export function parseChangedFiles(porcelainOut) {
  return porcelainOut
    .split("\n")
    .map((line) => line.slice(3).trim())
    .filter(Boolean)
    .map((p) => {
      const arrow = p.indexOf(" -> ");
      return arrow === -1 ? p : p.slice(arrow + 4);
    })
    .map((p) => p.replace(/^"|"$/g, ""));
}

/** Uncommitted changes, or null when git is unavailable. */
export function getChangedFiles(rootDir) {
  try {
    const out = execSync("git status --porcelain -uall", {
      cwd: rootDir,
      encoding: "utf8",
    });
    return parseChangedFiles(out);
  } catch {
    return null;
  }
}

/** Diff of one file against HEAD (staged + unstaged). Empty string on error. */
export function getFileDiff(rootDir, relPath) {
  try {
    return execSync(`git diff HEAD -- "${relPath}"`, {
      cwd: rootDir,
      encoding: "utf8",
    });
  } catch {
    return "";
  }
}

/** `export const NAME` start lines (1-based) in file content. */
function exportBlocks(fileContent) {
  const lines = fileContent.split("\n");
  const blocks = [];
  lines.forEach((line, i) => {
    const m = line.match(/^export\s+const\s+([A-Za-z_$][\w$]*)/);
    if (m) blocks.push({ name: m[1], start: i + 1 });
  });
  return blocks.map((b, i) => ({
    ...b,
    end: i + 1 < blocks.length ? blocks[i + 1].start - 1 : lines.length,
  }));
}

/**
 * Attribute each diff hunk to the export block containing it (via new-file
 * line ranges - robust against arbitrary hunk-header context), and collect
 * the changed lines per DUMMY_* export.
 *
 * Returns { exports, linesByExport, nonDummyChange } where exports lists
 * changed DUMMY_* names, linesByExport maps name -> { plus, minus } line
 * arrays, and nonDummyChange flags changes outside any DUMMY_* export
 * (helpers, writables) which conservatively affect every page.
 */
export function parseDummyDiff(diffText, fileContent) {
  const result = { exports: [], linesByExport: {}, nonDummyChange: false };
  if (!diffText || !diffText.trim()) return result;
  const blocks = exportBlocks(fileContent ?? "");
  const seen = new Set();
  const hunkRe = /^@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@/gm;
  let hunk;
  while ((hunk = hunkRe.exec(diffText)) !== null) {
    const newStart = parseInt(hunk[1], 10);
    const hunkEnd = diffText.indexOf("\n@@ ", hunk.index + 1);
    const body = diffText.slice(
      hunk.index + hunk[0].length,
      hunkEnd === -1 ? undefined : hunkEnd,
    );
    const plus = [];
    const minus = [];
    for (const line of body.split("\n")) {
      if (line.startsWith("+") && !line.startsWith("+++")) plus.push(line);
      else if (line.startsWith("-") && !line.startsWith("---"))
        minus.push(line);
    }
    if (plus.length === 0 && minus.length === 0) continue;
    const block = [...blocks].reverse().find((b) => b.start <= newStart);
    if (!block || !block.name.startsWith("DUMMY_")) {
      result.nonDummyChange = true;
      continue;
    }
    if (!seen.has(block.name)) {
      seen.add(block.name);
      result.exports.push(block.name);
      result.linesByExport[block.name] = { plus: [], minus: [] };
    }
    result.linesByExport[block.name].plus.push(...plus);
    result.linesByExport[block.name].minus.push(...minus);
  }
  return result;
}

/**
 * Field-level analysis of DUMMY_MOD_GROUPS changes.
 *
 * collections only renders name/displayName of its members, while mods
 * renders every field - so an installedAt-only change is mods-only.
 * Returns { anyChange, displayNameGroups, addedGroups, removedGroups }.
 */
export function analyseModGroupsDiff(plus, minus) {
  const analysis = {
    anyChange: plus.length > 0 || minus.length > 0,
    displayNameGroups: new Set(),
    addedGroups: new Set(),
    removedGroups: new Set(),
  };
  let currentGroup = null;
  const track = (line, changed) => {
    const nameMatch = line.match(GROUP_NAME_RE);
    if (nameMatch) {
      currentGroup = nameMatch[1];
      if (changed) {
        if (line.startsWith("+")) analysis.addedGroups.add(currentGroup);
        else analysis.removedGroups.add(currentGroup);
      }
      return;
    }
    if (!changed) return;
    if (GROUP_FIELD_RE.test(line)) {
      if (/displayName\s*:/.test(line) && currentGroup) {
        analysis.displayNameGroups.add(currentGroup);
      }
    } else if (GROUP_OTHER_FIELD_RE.test(line)) {
      // Other group-level field (installedAt, files, ...) - mods-only.
    }
  };
  // Context matters for attributing displayName changes to groups, so the
  // caller passes full hunk lines; here we only get +/- lines, therefore
  // group tracking uses +/- name lines (add/remove/rename) plus a fallback
  // below. DisplayName attribution without context is handled by scanning
  // the raw diff in planScreenshots via analyseModGroupsDiffWithContext.
  for (const line of plus) track(line, true);
  for (const line of minus) track(line, true);
  return {
    anyChange: analysis.anyChange,
    displayNameGroups: [...analysis.displayNameGroups],
    addedGroups: [...analysis.addedGroups],
    removedGroups: [...analysis.removedGroups],
  };
}

/**
 * Context-aware variant: scans raw diff text so displayName changes are
 * attributed to their enclosing group via surrounding `name:` lines.
 * Returns the set of group names with displayName changes.
 */
export function displayNameChangedGroups(diffText) {
  const groups = new Set();
  let currentGroup = null;
  let inModGroups = false;
  for (const line of diffText.split("\n")) {
    const hunk = line.match(/^@@.*@@\s*(.*)/);
    if (hunk) {
      inModGroups = /DUMMY_MOD_GROUPS/.test(hunk[1]);
      if (!inModGroups) {
        const other = hunk[1].match(/export\s+const\s+(DUMMY_[A-Z_]+)/);
        if (other) currentGroup = null;
      }
      continue;
    }
    if (!inModGroups) continue;
    const nameMatch = line.match(GROUP_NAME_RE);
    if (nameMatch) {
      currentGroup = nameMatch[1];
      continue;
    }
    if (/^[+-]\s{4}displayName\s*:/.test(line) && currentGroup) {
      groups.add(currentGroup);
    }
  }
  return [...groups];
}

/**
 * Collection member mod names from working-tree incognitoMode.ts content
 * (values of the DUMMY_COLLECTIONS arrays; keys excluded).
 */
export function parseCollectionMembers(fileContent) {
  if (!fileContent) return [];
  const start = fileContent.indexOf("DUMMY_COLLECTIONS");
  if (start === -1) return [];
  const rest = fileContent.slice(start);
  const nextExport = rest.slice(20).search(/\nexport\s+const\s+/);
  const block = nextExport === -1 ? rest : rest.slice(0, 20 + nextExport);
  const keys = new Set([...block.matchAll(/"([^"]+)"\s*:/g)].map((m) => m[1]));
  return [
    ...new Set(
      [...block.matchAll(/"([A-Za-z][A-Za-z0-9_]*)"/g)]
        .map((m) => m[1])
        .filter((name) => !keys.has(name)),
    ),
  ];
}

/**
 * Pure planner. Inputs are plain data (no I/O) so it is unit-testable.
 *
 * changedFiles: repo-relative paths from git status (null = git broken)
 * dummy: { diffText, content, isNew } for incognitoMode.ts when it changed
 * themes: e.g. ["light", "dark"]; force: keep everything
 * wizardOnly: WIZARD_PASS mode - pages never captured
 * exists: (relPath like "light/mods.png") => boolean
 *
 * Returns { wizard: {theme: {capture, reasons}}, pages: {theme: {page: {capture, reasons}}}, planned }
 */
export function planScreenshots({
  changedFiles,
  dummy,
  themes,
  force,
  wizardOnly,
  exists,
}) {
  const wizard = {};
  const pages = {};
  let planned = 0;
  const keep = (reasons) => {
    planned += 1;
    return { capture: true, reasons };
  };
  const skip = (reason) => ({ capture: false, reasons: [reason] });

  if (changedFiles === null && !force) {
    // Git unavailable - safest is to capture everything.
    for (const theme of themes) {
      wizard[theme] = keep(["git unavailable"]);
      pages[theme] = {};
      if (!wizardOnly) {
        for (const page of MAIN_PAGES)
          pages[theme][page] = keep(["git unavailable"]);
      }
    }
    return { wizard, pages, planned };
  }

  const files = changedFiles ?? [];
  const isNewFile = (rel) => !exists(rel);

  // Generic prefix matches (dummy file handled separately below).
  const genericReasons = (page) => {
    const watch = [...SHARED_SOURCES, ...(PAGE_SOURCES[page] ?? [])];
    return files.filter(
      (f) =>
        !f.startsWith("docs/screenshots/") &&
        f !== INCOGNITO_DUMMY_FILE &&
        watch.some((prefix) => f.startsWith(prefix)),
    );
  };

  // Dummy-file analysis (once, shared across themes).
  const dummyChanged = files.includes(INCOGNITO_DUMMY_FILE);
  let dummyParsed = null;
  let memberNames = [];
  if (dummyChanged && !force) {
    if (dummy?.isNew) {
      dummyParsed = { allExportsChanged: true };
    } else {
      dummyParsed = parseDummyDiff(dummy?.diffText ?? "", dummy?.content ?? "");
      memberNames = parseCollectionMembers(dummy?.content ?? "");
      dummyParsed.displayGroups = displayNameChangedGroups(
        dummy?.diffText ?? "",
      );
    }
  }

  // Which pages does the dummy change affect?
  const dummyReasons = { mods: [], collections: [], profiles: [] };
  if (dummyChanged && !force) {
    if (dummyParsed?.allExportsChanged) {
      for (const p of ALL_DUMMY_PAGES) {
        dummyReasons[p].push(`${INCOGNITO_DUMMY_FILE} (new file)`);
      }
    } else if (dummyParsed && dummyParsed.exports.length > 0) {
      for (const exp of dummyParsed.exports) {
        const tag = `${INCOGNITO_DUMMY_FILE}:${exp}`;
        if (exp === "DUMMY_MOD_GROUPS") {
          const lines = dummyParsed.linesByExport[exp];
          const analysis = analyseModGroupsDiff(lines.plus, lines.minus);
          if (!analysis.anyChange) continue;
          dummyReasons.mods.push(`${tag} (mod data)`);
          // Collections only renders name/displayName of its members.
          const touched = new Set([
            ...dummyParsed.displayGroups,
            ...analysis.addedGroups,
            ...analysis.removedGroups,
          ]);
          const memberTouched = [...touched].filter((g) =>
            memberNames.includes(g),
          );
          if (memberTouched.length > 0) {
            dummyReasons.collections.push(
              `${tag} (displayName of ${memberTouched.join(", ")})`,
            );
          }
        } else {
          for (const p of DUMMY_EXPORT_CONSUMERS[exp] ?? ALL_DUMMY_PAGES) {
            dummyReasons[p].push(tag);
          }
        }
      }
    } else if (dummyParsed?.nonDummyChange) {
      // Helpers/writables changed - affects the whole app shell.
      for (const p of [...ALL_DUMMY_PAGES, "wizard", "settings"]) {
        (dummyReasons[p] ??= []).push(`${INCOGNITO_DUMMY_FILE} (shared logic)`);
      }
    }
    // No attributable change (e.g. comment-only): no dummy reasons.
  }

  for (const theme of themes) {
    // Wizard: never consumes DUMMY_* data (empty list by design).
    const wReasons = force
      ? ["forced"]
      : [...genericReasons("wizard"), ...(dummyReasons.wizard ?? [])];
    if (force || wReasons.length > 0 || isNewFile(`${theme}/wizard.png`)) {
      const reasons = force
        ? ["forced"]
        : wReasons.length > 0
          ? wReasons
          : ["new file"];
      wizard[theme] = keep(reasons);
    } else {
      wizard[theme] = skip("no source changes");
    }

    pages[theme] = {};
    if (!wizardOnly) {
      for (const page of MAIN_PAGES) {
        const reasons = force
          ? ["forced"]
          : [...genericReasons(page), ...(dummyReasons[page] ?? [])];
        if (force || reasons.length > 0 || isNewFile(`${theme}/${page}.png`)) {
          pages[theme][page] = keep(
            force ? ["forced"] : reasons.length > 0 ? reasons : ["new file"],
          );
        } else {
          pages[theme][page] = skip("no source changes");
        }
      }
    }
  }
  return { wizard, pages, planned };
}
