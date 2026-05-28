# RoN Mod Manager

Cross-platform (Linux + Windows) GUI mod manager for Ready or Not.

![Preview](https://github.com/user-attachments/assets/53cda2a4-28bc-47a3-8d08-0dda8254fac4)

## Stack

- Tauri v2 (Rust backend)
- Svelte 5 + TypeScript (frontend)
- Tailwind CSS 4
- `npm` package management

## Development

```bash
make install
make dev
```

### Linux Development

```bash
make dev          # Wayland-compatible (software rendering)
make dev-xwayland # XWayland mode (full window state persistence)
```

See [docs/LINUX_WINDOW_PERSISTENCE.md](docs/LINUX_WINDOW_PERSISTENCE.md) for details on window state persistence on Linux/Wayland.

## Quality Checks

```bash
make lint-all        # all linters (frontend + backend)

# or individually:
make lint-frontend   # Prettier check + svelte-check
make lint-backend    # cargo fmt --check + clippy
make test            # vitest + cargo test
```

## Auto-Update Setup (GitHub Releases)

This project is configured for Tauri updater using:

- `src-tauri/tauri.conf.json` updater endpoint: `https://github.com/savagecore/RoNModManager/releases/latest/download/latest.json`
- `src-tauri/tauri.conf.json` application identifier: `uk.savagecore.ronmodmanager`
- `bundle.createUpdaterArtifacts = true`
- GitHub Actions release workflow signing env vars

Before shipping updater-enabled builds:

1. Generate updater keys once:

```bash
npm run tauri signer generate -w ~/.tauri/ronmodmanager.key
```

2. Copy the generated public key into `src-tauri/tauri.conf.json` `plugins.updater.pubkey`.
3. Add repository secrets:
   - `TAURI_SIGNING_PRIVATE_KEY`
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (optional if key has no password)

Without the correct public key and signing secrets, update checks/install will fail verification.

## Flatpak Packaging

Flatpak support is defined in:

- `packaging/flatpak/uk.savagecore.ronmodmanager.yml`
- `packaging/flatpak/uk.savagecore.ronmodmanager.desktop`

CI and release workflows build a `.flatpak` bundle and publish it as an artifact (and release asset on tags).

Local Flatpak build:

```bash
make flatpak-deps    # install runtimes once
make flatpak         # vendor → build → bundle → install
```

Run local bundle:

```bash
make flatpak-run
```

## Versioning

Version is stored in three files (`package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`).
Use `npm version` to bump all three at once:

```bash
npm version patch   # 0.0.0 -> 0.0.1
npm version minor   # 0.0.0 -> 0.1.0
npm version major   # 0.0.0 -> 1.0.0
```

The `version` hook syncs `tauri.conf.json` and `Cargo.toml` and stages them before npm creates the version commit and tag, so all three files land in a single commit.

## License

MIT

## Modpack Export & Hosting

See [docs/HOSTING_MODPACKS.md](docs/HOSTING_MODPACKS.md) for instructions on exporting, self-hosting, and sharing modpacks.
