## Hosting and Sharing Modpacks

You can export your installed mods and collections as a self-hostable modpack for others to use with RoN Mod Manager.

### How to Export a Modpack

1. Go to **Settings > Export Modpack**.
2. Fill in the modpack name, version, description, and author in the export dialog.
3. The app will export a `modpack.json` manifest and a `mods/` directory of mod files to your `~/Downloads` folder by default.

### Directory Structure for Hosting

To self-host a modpack, upload the following to your web server or file host:

```
modpack-root/
  modpack.json   # The exported manifest
  mods/          # Directory containing all .pak mod files
```

- The manifest JSON describes collections, mod subscriptions (mod.io, Nexus, manual), and metadata.
- The `mods/` directory should contain all referenced mod files.

### Using a Hosted Modpack

1. Upload the exported files to your server (keep the directory structure).
2. Share the URL to the modpack root (e.g. `https://example.com/modpack`) or directly to `modpack.json`.
3. In RoN Mod Manager, paste this URL in the **Modpack URL** field and click **Sync**.

The app will read the manifest, subscribe to mod.io/Nexus mods, and download/enable the correct mods and collections.

**Tip:** You can update the manifest and re-upload to update the modpack for all users.

### One-click install link

Anyone with RoN Mod Manager installed can open your modpack directly via a `ronmm://` URL:

```
ronmm://modpack/https://example.com/modpack
```

The app resolves the base URL to `modpack.json` automatically. Share this link in Discord, a website, or a README - clicking it will prompt the user to sync the modpack immediately.

### Publishing via SFTP

RoN Mod Manager has a built-in SFTP sync feature so you can publish without manually uploading files. Configure your SFTP destination (host, path, credentials) once and use **Sync to SFTP** to push the last exported modpack - both `modpack.json` and the `mods/` directory - directly from within the app.

> **Note:** SFTP always syncs whichever modpack was most recently exported. If you work with multiple modpacks, make sure to update the destination path before syncing to avoid overwriting the wrong remote directory.
