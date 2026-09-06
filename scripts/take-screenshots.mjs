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
 */
import { spawn, execSync } from "child_process";
import { fileURLToPath } from "url";
import path from "path";
import fs from "fs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.join(__dirname, "..");
const OUT = path.join(ROOT, "docs", "screenshots");

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

for (const tool of ["xdotool", "convert", "import"]) {
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

function killApp(app) {
  return new Promise((resolve) => {
    app.kill();
    // Give it a moment to release the window
    setTimeout(resolve, 500);
  });
}

function waitForWindow(timeoutMs = 20000) {
  return new Promise((resolve, reject) => {
    const start = Date.now();
    const interval = setInterval(() => {
      try {
        const ids = x(`xdotool search --name "RoN Mod Manager"`).trim();
        if (ids) {
          clearInterval(interval);
          resolve(ids.split("\n").at(-1));
        }
      } catch {
        // not ready yet
      }
      if (Date.now() - start > timeoutMs) {
        clearInterval(interval);
        reject(new Error("Window did not appear in time."));
      }
    }, 500);
  });
}

async function launchApp(theme, wizardPass) {
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

  const wid = await waitForWindow();
  console.log(`Window ID: ${wid}`);
  x(`xdotool windowsize ${wid} 1280 840`);
  x(`xdotool windowfocus --sync ${wid}`);
  x(`xdotool windowraise ${wid}`);

  // Wait for the app to load
  await new Promise((r) => setTimeout(r, 4000));

  return { app, wid };
}

async function capture(wid, name, theme) {
  const dir = path.join(OUT, theme);
  fs.mkdirSync(dir, { recursive: true });
  const file = path.join(dir, `${name}.png`);
  execSync(`DISPLAY=${display} import -window ${wid} "${file}"`);
  const borderColor = theme === "dark" ? "#ffffff" : "#333333";
  execSync(`convert "${file}" -bordercolor "${borderColor}" -border 40 "${file}"`);
  console.log(`  ✓  ${name}`);
}

// For each theme: capture main pages, then restart the app to capture the wizard.
// The dev server keeps running across restarts so the frontend is never rebuilt.
for (const theme of themes) {
  console.log(`\n── ${theme.toUpperCase()} ──`);

  if (!wizardOnly) {
    // Main pages pass
    const { app, wid } = await launchApp(theme, false);
    const pages = ["mods", "collections", "profiles", "settings"];
    for (let i = 0; i < pages.length; i++) {
      const name = pages[i];
      x(`xdotool key --window ${wid} --clearmodifiers ${i + 1}`);
      await new Promise((r) => setTimeout(r, 1200));
      await capture(wid, name, theme);
    }
    await killApp(app);
  }

  // Wizard pass: restart the app with the wizard forced on
  const { app, wid } = await launchApp(theme, true);
  await capture(wid, "wizard", theme);
  await killApp(app);
}

vite.kill();

// Restore original config
if (originalConfig) {
  fs.writeFileSync(configFile, originalConfig);
  console.log("Restored original config");
}

console.log(`\nSaved to ${path.relative(ROOT, OUT)}/`);