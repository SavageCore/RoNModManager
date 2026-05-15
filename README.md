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
npm install
npm run dev
npm run tauri dev
```

### Linux Development

```bash
# Native Wayland (window position won't persist)
npm run dev:linux

# XWayland mode (full window state persistence)
npm run dev:xwayland
```

See [docs/LINUX_WINDOW_PERSISTENCE.md](docs/LINUX_WINDOW_PERSISTENCE.md) for details on window state persistence on Linux/Wayland.

## Quality Checks

```bash
npm run lint
npm run check
npm run test:unit

cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
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
flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
flatpak install -y flathub org.gnome.Platform//47 org.gnome.Sdk//47 org.freedesktop.Sdk.Extension.node22//24.08 org.freedesktop.Sdk.Extension.rust-stable//24.08

cd src-tauri
cargo vendor vendor
cd ..
flatpak-builder --force-clean --user --install-deps-from=flathub --repo=flatpak-repo build-dir packaging/flatpak/uk.savagecore.ronmodmanager.yml
flatpak build-bundle flatpak-repo ronmodmanager.flatpak uk.savagecore.ronmodmanager
```

Install and run local bundle:

```bash
flatpak install --user --bundle ronmodmanager.flatpak
flatpak run uk.savagecore.ronmodmanager
```

## License

MIT
