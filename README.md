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

## License

MIT
