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
 * Only screenshots whose pixels actually changed are kept: each capture is
 * written to a staging dir and promoted over the previous file only when an
 * ImageMagick RMSE comparison (with a small fuzz for antialiasing noise)
 * reports a difference. Unchanged files are left untouched so `git status`
 * stays clean. Set SCREENSHOT_FORCE=1 to overwrite everything.
 */
import { spawn, execSync } from "child_process";
import { fileURLToPath } from "url";
import path from "path";
import fs from "fs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.join(__dirname, "..");
const OUT = path.join(ROOT, "docs", "screenshots");
const STAGING = path.join(OUT, ".tmp");
// Normalized RMSE below this counts as "unchanged". Observed run-to-run
// noise for identical pages sits around 0.008-0.011, so the default keeps
// clear of that. Override with SCREENSHOT_FUZZ=0.005 etc. to tune sensitivity.
const FUZZ_THRESHOLD = parseFloat(process.env.SCREENSHOT_FUZZ ?? "0.02");
const forceOverwrite = process.env.SCREENSHOT_FORCE === "1";
if (forceOverwrite) console.log("Force mode: all screenshots will be kept.");

// Prefer `magick` subcommands on ImageMagick v7 (the legacy `convert` /
// `import` / `compare` shims print a deprecation warning on every call).
let importCmd = "import";
let convertCmd = "convert";
let compareCmd = "compare";
try {
  execSync("which magick", { stdio: "ignore" });
  importCmd = "magick import";
  convertCmd = "magick";
  compareCmd = "magick compare";
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
  useMagick ? "magick" : "compare",
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
      try {
        const ids = x(`xdotool search --name "RoN Mod Manager"`).trim();
        if (!ids) {
          clearInterval(interval);
          resolve();
          return;
        }
      } catch {}
      if (Date.now() - start > timeoutMs) {
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

let changedCount = 0;
let totalCount = 0;

function imageDiff(oldFile, newFile) {
  try {
    // `compare` exits 0 when identical, 1 when different; metric goes to stderr.
    const out = execSync(
      `${compareCmd} -metric RMSE -fuzz 1% "${oldFile}" "${newFile}" null: 2>&1`,
      { encoding: "utf8" },
    );
    return { metric: parseMetric(out), raw: out.trim() };
  } catch (err) {
    const out = (err.stdout ?? "").toString() + (err.stderr ?? "").toString();
    if (!out.trim())
      return { metric: err.status === 0 ? 0 : Number.NaN, raw: "" };
    return { metric: parseMetric(out), raw: out.trim() };
  }
}

function parseMetric(out) {
  const m = out.match(/\(([0-9.]+)\)/);
  return m ? parseFloat(m[1]) : Number.NaN;
}

// One-line human reason when no RMSE metric could be parsed (e.g. the
// images have different dimensions, so compare errors out).
function diffDetail(raw) {
  const first = (raw.split("\n").pop() ?? "").trim();
  if (/widths? or heights? differ/i.test(first)) return "size differs";
  if (!first) return "compare failed";
  return first.length > 120 ? first.slice(0, 117) + "..." : first;
}

async function capture(wid, name, theme) {
  totalCount++;
  const dir = path.join(OUT, theme);
  fs.mkdirSync(dir, { recursive: true });
  const stagingDir = path.join(STAGING, theme);
  fs.mkdirSync(stagingDir, { recursive: true });
  const file = path.join(dir, `${name}.png`);
  const staged = path.join(stagingDir, `${name}.png`);
  execSync(`DISPLAY=${display} ${importCmd} -window ${wid} "${staged}"`);
  const borderColor = theme === "dark" ? "#ffffff" : "#333333";
  execSync(
    `${convertCmd} "${staged}" -bordercolor "${borderColor}" -border 40 "${staged}"`,
  );
  if (!fs.existsSync(file)) {
    fs.renameSync(staged, file);
    changedCount++;
    console.log(`  ✓  ${name} (new)`);
    return true;
  }
  if (forceOverwrite) {
    fs.renameSync(staged, file);
    changedCount++;
    console.log(`  ✓  ${name} (forced)`);
    return true;
  }
  const { metric, raw } = imageDiff(file, staged);
  const label = Number.isNaN(metric)
    ? diffDetail(raw)
    : `RMSE ${metric.toFixed(4)}`;
  if (metric < FUZZ_THRESHOLD) {
    fs.rmSync(staged);
    console.log(`  =  ${name} (unchanged, ${label})`);
    return false;
  } else {
    fs.renameSync(staged, file);
    changedCount++;
    console.log(`  ✓  ${name} (updated, ${label})`);
    return true;
  }
}

// Capture the wizard page for each theme. When the first theme comes back
// unchanged, the remaining themes are skipped - theme pairs only ever differ
// when the first one changed. Force mode still captures everything.
let wizardChanged = forceOverwrite;
for (const theme of themes) {
  if (!wizardChanged && !forceOverwrite && theme !== themes[0]) {
    console.log(
      `\n── ${theme.toUpperCase()} WIZARD ──\n  - skipped (${themes[0]} unchanged)`,
    );
    continue;
  }
  console.log(`\n── ${theme.toUpperCase()} WIZARD ──`);
  const { app, wid } = await launchApp(theme, true);
  const changed = await capture(wid, "wizard", theme);
  wizardChanged = wizardChanged || changed;
  await killApp(app);
}

if (!wizardOnly) {
  // Same group-skip for the main pages: if the first theme had zero changes,
  // the second theme launch is skipped entirely.
  let groupChanged = forceOverwrite;
  for (const theme of themes) {
    if (!groupChanged && !forceOverwrite && theme !== themes[0]) {
      console.log(
        `\n── ${theme.toUpperCase()} ──\n  - skipped (${themes[0]} unchanged)`,
      );
      continue;
    }
    console.log(`\n── ${theme.toUpperCase()} ──`);
    const { app, wid } = await launchApp(theme, false);
    const pages = ["mods", "collections", "profiles", "settings"];
    for (let i = 0; i < pages.length; i++) {
      const name = pages[i];
      x(`xdotool key --window ${wid} --clearmodifiers ${i + 1}`);
      await new Promise((r) => setTimeout(r, 1200));
      const changed = await capture(wid, name, theme);
      if (theme === themes[0]) groupChanged = groupChanged || changed;
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

console.log(
  `\nSaved to ${path.relative(ROOT, OUT)}/ (${changedCount}/${totalCount} changed)`,
);
