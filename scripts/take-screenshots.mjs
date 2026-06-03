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
 *   node scripts/take-screenshots.mjs
 */
import { spawn, execSync } from "child_process";
import { fileURLToPath } from "url";
import path from "path";
import fs from "fs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.join(__dirname, "..");
const THEME = process.env.SCREENSHOT_THEME ?? "light";
const OUT = path.join(ROOT, "docs", "screenshots", THEME);

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

fs.mkdirSync(OUT, { recursive: true });

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
  process.exit(1);
}
console.log("Vite ready.");

// Launch the debug binary - it connects to localhost:1420.
// SCREENSHOT_MODE suppresses devtools and activates incognito + number-key nav.
const app = spawn(appBinary, [], {
  env: {
    ...process.env,
    DISPLAY: display,
    GDK_BACKEND: "x11",
    SCREENSHOT_MODE: "1",
    SCREENSHOT_THEME: THEME,
    WEBKIT_DISABLE_DMABUF_RENDERER: "1",
    LIBGL_ALWAYS_SOFTWARE: "1",
  },
  stdio: "ignore",
});
app.on("error", (e) => {
  vite.kill();
  console.error(`Failed to start app: ${e.message}`);
  process.exit(1);
});

function x(cmd) {
  return execSync(`DISPLAY=${display} ${cmd}`, { encoding: "utf8" });
}

// Wait for the window to appear (up to 20 s)
let wid = null;
for (let i = 0; i < 40 && !wid; i++) {
  await new Promise((r) => setTimeout(r, 500));
  try {
    const ids = x(`xdotool search --name "RoN Mod Manager"`).trim();
    if (ids) wid = ids.split("\n").at(-1);
  } catch {}
}
if (!wid) {
  app.kill();
  vite.kill();
  console.error("Window did not appear in time.");
  process.exit(1);
}
console.log(`Window ID: ${wid}`);

x(`xdotool windowsize ${wid} 1280 840`);
x(`xdotool windowfocus --sync ${wid}`);
x(`xdotool windowraise ${wid}`);

// Wait for the app to load and auto-activate incognito
await new Promise((r) => setTimeout(r, 4000));

// Navigate each page via number keys wired to goto() in screenshot mode
const pages = ["mods", "collections", "profiles", "settings"];

for (let i = 0; i < pages.length; i++) {
  const name = pages[i];
  x(`xdotool key --window ${wid} --clearmodifiers ${i + 1}`);
  await new Promise((r) => setTimeout(r, 1200));

  const file = path.join(OUT, `${name}.png`);
  execSync(`DISPLAY=${display} import -window ${wid} "${file}"`);
  const borderColor = THEME === "dark" ? "#ffffff" : "#333333";
  execSync(
    `convert "${file}" -bordercolor "${borderColor}" -border 40 "${file}"`,
  );
  console.log(`  ✓  ${name}`);
}

app.kill();
vite.kill();
console.log(`\nSaved to ${path.relative(ROOT, OUT)}/`);
