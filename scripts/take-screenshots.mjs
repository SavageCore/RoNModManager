#!/usr/bin/env node
/**
 * Take screenshots of every main page using xdotool + XWayland.
 * Requires: xdotool, imagemagick
 *
 * Starts the Vite dev server (localhost:1420) then launches the debug binary
 * so the latest frontend code is always used - no Tauri rebuild needed after
 * frontend-only changes. Rebuild the debug binary with `make screenshots-build`
 * when Rust code changes.
 *
 * Usage:
 *   node scripts/take-screenshots.mjs            # all pages, light + dark
 *   WIZARD_PASS=1 node scripts/take-screenshots.mjs  # just the wizard welcome page
 *
 * Code-based planning only - there is deliberately NO pixel/RMSE image
 * comparison anywhere. Before anything launches, screenshot-plan.mjs maps
 * uncommitted source changes to the pages they can affect (page sources,
 * shared chrome, and DUMMY_* export consumers with field-level precision
 * for DUMMY_MOD_GROUPS). Skipped screenshots are never taken at all: no
 * app launch, no window capture. Set SCREENSHOT_FORCE=1 (or run
 * `make screenshots-force`) to take everything.
 */
import { spawn, execSync } from "child_process";
import { fileURLToPath } from "url";
import path from "path";
import fs from "fs";
import {
  INCOGNITO_DUMMY_FILE,
  MAIN_PAGES,
  getChangedFiles,
  getFileDiff,
  planScreenshots,
} from "./screenshot-plan.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.join(__dirname, "..");
const OUT = path.join(ROOT, "docs", "screenshots");
const STAGING = path.join(OUT, ".tmp");
const forceOverwrite = process.env.SCREENSHOT_FORCE === "1";
if (forceOverwrite) console.log("Force mode: all screenshots will be taken.");

// Prefer `magick` subcommands on ImageMagick v7 (the legacy `convert` /
// `import` shims print a deprecation warning on every call).
let importCmd = "import";
let convertCmd = "convert";
try {
  execSync("which magick", { stdio: "ignore" });
  importCmd = "magick import";
  convertCmd = "magick";
} catch {}

const appBinary = path.join(
  ROOT,
  "src-tauri",
  "target",
  "debug",
  "ronmodmanager",
);
if (!fs.existsSync(appBinary)) {
  console.error(
    "Debug binary not found. Build it first:\n  make screenshots-build",
  );
  process.exit(1);
}
console.log(`Binary: ${path.relative(ROOT, appBinary)}`);

const useMagick = importCmd.startsWith("magick");
for (const tool of [
  "xdotool",
  useMagick ? "magick" : "convert",
  useMagick ? "magick" : "import",
]) {
  try {
    execSync(`which ${tool}`, { stdio: "ignore" });
  } catch {
    console.error(`'${tool}' not found. Install xdotool and imagemagick.`);
    process.exit(1);
  }
}

// Find an available X11 display (XWayland is typically :0 or :1 on KDE Plasma)
let display = process.env.DISPLAY ?? null;
if (!display) {
  for (const d of [":1", ":0", ":2"]) {
    try {
      execSync(`DISPLAY=${d} xdpyinfo`, { stdio: "ignore" });
      display = d;
      break;
    } catch {}
  }
}
if (!display) {
  console.error(
    "No X11 display found. Make sure XWayland is running " +
      "(on KDE Plasma it starts automatically).",
  );
  process.exit(1);
}
console.log(`Display: ${display}`);

const themes = process.env.SCREENSHOT_THEME
  ? [process.env.SCREENSHOT_THEME]
  : ["light", "dark"];
const wizardOnly = process.env.WIZARD_PASS === "1";

// --- Plan first: decide everything from source changes, before launching ---
const changedFiles = getChangedFiles(ROOT);
let dummyContent = "";
let dummyIsNew = false;
try {
  dummyContent = fs.readFileSync(path.join(ROOT, INCOGNITO_DUMMY_FILE), "utf8");
} catch {}
try {
  const st = execSync(`git status --porcelain -- "${INCOGNITO_DUMMY_FILE}"`, {
    cwd: ROOT,
    encoding: "utf8",
  });
  dummyIsNew = /^[?A]/.test(st.trim());
} catch {}
const plan = planScreenshots({
  changedFiles,
  dummy: {
    diffText:
      changedFiles?.includes(INCOGNITO_DUMMY_FILE) && !dummyIsNew
        ? getFileDiff(ROOT, INCOGNITO_DUMMY_FILE)
        : "",
    content: dummyContent,
    isNew: dummyIsNew,
  },
  themes,
  force: forceOverwrite,
  wizardOnly,
  exists: (rel) => fs.existsSync(path.join(OUT, rel)),
});

console.log("\nPlan:");
let planTotal = 0;
for (const theme of themes) {
  const w = plan.wizard[theme];
  console.log(
    `  ${theme} wizard: ${w.capture ? "take" : "skip"} (${w.reasons.join("; ")})`,
  );
  if (w.capture) planTotal++;
  if (wizardOnly) continue;
  for (const page of MAIN_PAGES) {
    const p = plan.pages[theme]?.[page];
    if (!p) continue;
    console.log(
      `  ${theme} ${page}: ${p.capture ? "take" : "skip"} (${p.reasons.join("; ")})`,
    );
    if (p.capture) planTotal++;
  }
}
if (planTotal === 0) {
  console.log(
    "\nNothing to do - no uncommitted changes affect any screenshot.\n" +
      "Planning reads the working tree only, so a change that is already\n" +
      "committed needs SCREENSHOT_FORCE=1 (make screenshots-force).",
  );
  process.exit(0);
}

// Config file path
const configDir = process.env.HOME + "/.config/ronmodmanager-dev";
const configFile = configDir + "/config.json";
let originalConfig = null;

// Backup and modify config to show wizard (only needed for wizard pass)
if (wizardOnly) {
  try {
    if (fs.existsSync(configFile)) {
      originalConfig = fs.readFileSync(configFile, "utf8");
      const config = JSON.parse(originalConfig);
      config.game_path = null;
      config.setup_wizard_complete = false;
      fs.writeFileSync(configFile, JSON.stringify(config, null, 2));
      console.log("Modified config to show wizard");
    }
  } catch (err) {
    console.error("Failed to modify config:", err.message);
    process.exit(1);
  }
}

// Start the Vite dev server so the debug binary always loads the latest
// frontend code without requiring a full Tauri rebuild.
console.log("Starting Vite dev server...");
const vite = spawn("npm", ["run", "dev"], {
  cwd: ROOT,
  stdio: "ignore",
  detached: false,
});
vite.on("error", (e) => {
  console.error(`Vite failed to start: ${e.message}`);
  if (originalConfig) fs.writeFileSync(configFile, originalConfig);
  process.exit(1);
});

// Wait for Vite to be ready (up to 30 s)
let viteReady = false;
for (let i = 0; i < 60 && !viteReady; i++) {
  await new Promise((r) => setTimeout(r, 500));
  try {
    execSync("curl -sf http://localhost:1420", { stdio: "ignore" });
    viteReady = true;
  } catch {}
}
if (!viteReady) {
  vite.kill();
  console.error("Vite dev server did not become ready in time.");
  if (originalConfig) fs.writeFileSync(configFile, originalConfig);
  process.exit(1);
}
console.log("Vite ready.");

function x(cmd) {
  return execSync(`DISPLAY=${display} ${cmd}`, { encoding: "utf8" });
}

function windowIds() {
  try {
    const ids = x(`xdotool search --name "RoN Mod Manager"`).trim();
    return ids ? new Set(ids.split("\n")) : new Set();
  } catch {
    return new Set();
  }
}

function waitForWindowToDisappear(timeoutMs = 15000) {
  return new Promise((resolve) => {
    const start = Date.now();
    const interval = setInterval(() => {
      let gone = false;
      try {
        gone = !x(`xdotool search --name "RoN Mod Manager"`).trim();
      } catch (err) {
        // xdotool exits 1 when no windows match - that means gone.
        gone = err?.status === 1;
      }
      if (gone || Date.now() - start > timeoutMs) {
        clearInterval(interval);
        resolve();
      }
    }, 200);
  });
}

async function killApp(app) {
  app.kill();
  await waitForWindowToDisappear();
  try {
    execSync(
      `DISPLAY=${display} xdotool search --name "RoN Mod Manager" windowkill`,
      { stdio: "ignore" },
    );
  } catch {}
  // The next launch must not mistake this window for the fresh one.
  await waitForWindowToDisappear();
}

function waitForWindow(timeoutMs = 20000, knownIds = new Set()) {
  return new Promise((resolve, reject) => {
    const start = Date.now();
    const interval = setInterval(() => {
      try {
        const ids = x(`xdotool search --name "RoN Mod Manager"`).trim();
        if (ids) {
          // Prefer a window ID we have not seen before; fall back to the
          // last match only once the old windows are gone.
          const fresh = ids.split("\n").find((id) => !knownIds.has(id));
          if (fresh) {
            clearInterval(interval);
            resolve(fresh);
            return;
          }
          if (knownIds.size === 0) {
            clearInterval(interval);
            resolve(ids.split("\n").at(-1));
            return;
          }
        }
      } catch {}
      if (Date.now() - start > timeoutMs) {
        clearInterval(interval);
        reject(new Error("Window did not appear in time."));
      }
    }, 500);
  });
}

async function launchApp(theme, wizardPass) {
  const knownIds = windowIds();
  const app = spawn(appBinary, [], {
    env: {
      ...process.env,
      DISPLAY: display,
      GDK_BACKEND: "x11",
      WEBKIT_DISABLE_DMABUF_RENDERER: "1",
      LIBGL_ALWAYS_SOFTWARE: "1",
      SCREENSHOT_MODE: "1",
      SCREENSHOT_THEME: theme,
      ...(wizardPass ? { WIZARD_SCREENSHOT: "1" } : {}),
    },
    stdio: "ignore",
  });
  app.on("error", (e) => {
    vite.kill();
    console.error(`Failed to start app: ${e.message}`);
    process.exit(1);
  });

  const wid = await waitForWindow(20000, knownIds);
  console.log(`Window ID: ${wid}`);

  for (let attempt = 0; attempt < 5; attempt++) {
    try {
      x(`xdotool windowsize ${wid} 1280 840`);
      x(`xdotool windowfocus --sync ${wid}`);
      x(`xdotool windowraise ${wid}`);
      break;
    } catch (err) {
      if (attempt === 4) throw err;
      await new Promise((r) => setTimeout(r, 500));
    }
  }

  await new Promise((r) => setTimeout(r, 4000));

  return { app, wid };
}

let takenCount = 0;

// Capture direct to the final file - the plan already decided this shot is
// needed, so there is no staging, no compare, no discard.
function capture(wid, name, theme) {
  const dir = path.join(OUT, theme);
  fs.mkdirSync(dir, { recursive: true });
  const file = path.join(dir, `${name}.png`);
  execSync(`DISPLAY=${display} ${importCmd} -window ${wid} "${file}"`);
  const borderColor = theme === "dark" ? "#ffffff" : "#333333";
  execSync(
    `${convertCmd} "${file}" -bordercolor "${borderColor}" -border 40 "${file}"`,
  );
  takenCount++;
  console.log(`  ✓  ${name}`);
}

const PAGE_KEYS = { mods: "1", collections: "2", profiles: "3", settings: "4" };

// Wizard passes: only themes the plan selected are launched at all.
for (const theme of themes) {
  const w = plan.wizard[theme];
  if (!w.capture) {
    console.log(
      `\n── ${theme.toUpperCase()} WIZARD ──\n  - skipped (${w.reasons.join("; ")})`,
    );
    continue;
  }
  console.log(`\n── ${theme.toUpperCase()} WIZARD ──`);
  const { app, wid } = await launchApp(theme, true);
  await capture(wid, "wizard", theme);
  await killApp(app);
}

if (!wizardOnly) {
  // One launch per theme, then jump straight to each planned page.
  // Unplanned pages are never visited - no sequential walk-through.
  for (const theme of themes) {
    const entries = MAIN_PAGES.filter((p) => plan.pages[theme]?.[p]?.capture);
    if (entries.length === 0) {
      const why =
        plan.pages[theme]?.mods?.reasons.join("; ") ?? "no source changes";
      console.log(`\n── ${theme.toUpperCase()} ──\n  - skipped (${why})`);
      continue;
    }
    console.log(`\n── ${theme.toUpperCase()} ──`);
    const { app, wid } = await launchApp(theme, false);
    for (const name of entries) {
      x(`xdotool key --window ${wid} --clearmodifiers ${PAGE_KEYS[name]}`);
      await new Promise((r) => setTimeout(r, 1200));
      await capture(wid, name, theme);
    }
    await killApp(app);
  }
}

vite.kill();
fs.rmSync(STAGING, { recursive: true, force: true });

if (originalConfig) {
  fs.writeFileSync(configFile, originalConfig);
  console.log("Restored original config");
}

console.log(`\nSaved to ${path.relative(ROOT, OUT)}/ (${takenCount} taken)`);
