import { defineConfig } from "vitest/config";
import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import type { Plugin } from "vite";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// Vitest resolves modules with node conditions, which makes `svelte` (and
// `svelte/store`) resolve to their server builds - mounting components then
// fails with `lifecycle_function_unavailable`. Force browser conditions for
// test runs so @testing-library/svelte mounts client components.
// @ts-expect-error process is a nodejs global
const isVitest = process.env.VITEST;

// When vite-plugin-svelte can't find cached CSS for a virtual style module
// (e.g. no <style> block, or cache not yet populated on startup/HMR), its
// load hook returns undefined and Vite falls back to the raw .svelte source.
// Tailwind's pre-transform then receives JS imports as CSS and hard-errors.
// This guard intercepts those cases and returns empty CSS instead.
const svelteCssGuard: Plugin = {
  name: "svelte-css-guard",
  enforce: "pre",
  transform(code, id) {
    if (
      id.includes("?svelte") &&
      id.includes("&lang.css") &&
      code.includes("<script")
    ) {
      return { code: "", map: null };
    }
  },
};

export default defineConfig({
  plugins: [svelteCssGuard, tailwindcss(), sveltekit()],
  clearScreen: false,
  resolve: isVitest ? { conditions: ["browser"] } : undefined,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: [
        "**/src-tauri/**",
        "**/build-dir/**",
        "**/.flatpak/**",
        "**/.flatpak-builder/**",
        "**/test-build-dir/**",
      ],
    },
  },
  test: {
    environment: "jsdom",
    include: ["tests/unit/**/*.test.ts"],
    setupFiles: ["tests/setup.ts"],
    coverage: {
      provider: "v8",
      reporter: ["text", "lcov"],
      thresholds: {
        statements: 78,
        branches: 72,
        functions: 55,
        lines: 80,
      },
    },
  },
});
