import { defineConfig } from "vite";
import monkey from "vite-plugin-monkey";

export default defineConfig(({ mode }) => ({
  plugins: [
    monkey({
      entry: "src/main.ts",
      userscript: {
        name: "Ready or Not Mod Manager Companion",
        namespace: "savagecore/ron-mod-manager",
        version: "1.0.0",
        match: [
          "https://www.nexusmods.com/readyornot/mods/*",
          "https://mod.io/g/readyornot/m/*",
        ],
        grant: [],
        updateURL:
          "https://github.com/SavageCore/RoNModManager/releases/latest/download/ron-mod-manager-userscript.user.js",
        downloadURL:
          "https://github.com/SavageCore/RoNModManager/releases/latest/download/ron-mod-manager-userscript.user.js",
      },
      build: {
        fileName:
          mode === "development"
            ? "ron-mod-manager-userscript.dev.user.js"
            : "ron-mod-manager-userscript.user.js",
      },
    }),
  ],
}));
