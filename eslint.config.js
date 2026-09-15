import { defineConfig } from "eslint/config";
import packageJson from "eslint-plugin-package-json";

export default defineConfig([
  {
    ignores: [
      "**/node_modules/**",
      "**/coverage/**",
      "**/build/**",
      "**/build-dir/**",
      "**/.svelte-kit/**",
      "**/src-tauri/target/**",
      "**/src-tauri/vendor/**",
      "**/src-tauri/.cargo/**",
      "**/.flatpak-builder/**",
      "**/flatpak-repo/**",
      "**/.opencode/**",
      "**/.commandcode/**",
    ],
  },
  {
    files: ["**/package.json"],
    plugins: {
      "package-json": packageJson,
    },
    extends: ["package-json/recommended"],
  },
]);
