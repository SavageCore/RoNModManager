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

- `src-tauri/tauri.conf.json` updater endpoint: `https://github.com/savagecore/RoNModManager/releases/latest/download/latest.json`
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

## License

MIT
