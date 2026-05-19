## Hosting and Sharing Modpacks

You can export your installed mods and collections as a self-hostable modpack for others to use with RoN Mod Manager.

### How to Export a Modpack

1. Go to **Settings > Export Modpack**.
2. Fill in the modpack name, version, description, and author in the export dialog.
3. The app will export a `.json` manifest and a directory of mod files to your `~/Downloads` folder by default.

### Directory Structure for Hosting

To self-host a modpack, upload the following to your web server or file host:

```
modpack-root/
  ronmod-export-YYYY-MM-DD.json   # The exported manifest
  mods/                          # Directory containing all .pak mod files
```

- The manifest JSON describes collections, mod subscriptions (mod.io, Nexus, manual), and metadata.
- The `mods/` directory should contain all referenced mod files.

### Using a Hosted Modpack

1. Upload the exported files to your server (keep the directory structure).
2. Share the URL to the `.json` manifest (e.g. `https://example.com/modpack/ronmod-export-2026-05-16.json`).
3. In RoN Mod Manager, paste this URL in the **Modpack URL** field and click **Sync**.

The app will read the manifest, subscribe to mod.io/Nexus mods, and download/enable the correct mods and collections.

**Tip:** You can update the manifest and re-upload to update the modpack for all users.
