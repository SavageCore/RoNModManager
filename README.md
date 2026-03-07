# RoN Mod Manager

Cross-platform (Linux + Windows) GUI mod manager for Ready or Not.

## Stack

- Tauri v2 (Rust backend)
- Svelte 5 + TypeScript (frontend)
- Tailwind CSS 4
- `pnpm` package management

## Development

```bash
pnpm install
pnpm dev
pnpm tauri dev
```

## Quality Checks

```bash
pnpm lint
pnpm format:check
pnpm check
pnpm test:unit

cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

## Auto-Update Setup (GitHub Releases)

This project is configured for Tauri updater using:

- `src-tauri/tauri.conf.json` updater endpoint: `https://savagecore.uk/ronmodmanager/latest.json`
- `bundle.createUpdaterArtifacts = true`
- GitHub Actions release workflow signing env vars

Before shipping updater-enabled builds:

1. Generate updater keys once:

```bash
pnpm tauri signer generate -w ~/.tauri/ronmodmanager.key
```

2. Copy the generated public key into `src-tauri/tauri.conf.json` `plugins.updater.pubkey`.
3. Add repository secrets:
   - `TAURI_SIGNING_PRIVATE_KEY`
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (optional if key has no password)

Without the correct public key and signing secrets, update checks/install will fail verification.

## Flatpak Packaging

Flatpak support is defined in:

- `packaging/flatpak/com.savagecore.ronmodmanager.yml`
- `packaging/flatpak/com.savagecore.ronmodmanager.desktop`

CI and release workflows build a `.flatpak` bundle and publish it as an artifact (and release asset on tags).

Local Flatpak build:

```bash
sudo apt-get update
sudo apt-get install -y flatpak flatpak-builder
flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
flatpak install -y flathub org.gnome.Platform//47 org.gnome.Sdk//47 org.freedesktop.Sdk.Extension.node22//24.08 org.freedesktop.Sdk.Extension.rust-stable//24.08

flatpak-builder --force-clean --user --install-deps-from=flathub --repo=flatpak-repo build-dir packaging/flatpak/com.savagecore.ronmodmanager.yml
flatpak build-bundle flatpak-repo ronmodmanager.flatpak com.savagecore.ronmodmanager
```

Install and run local bundle:

```bash
flatpak install --user --bundle ronmodmanager.flatpak
flatpak run com.savagecore.ronmodmanager
```

## License

MIT
