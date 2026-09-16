# RoNModManager

## Commit hooks are slow

The lefthook pre-commit hook runs prettier, `cargo fmt --check` and
`cargo clippy`, which can take several minutes (cold cargo builds especially).
When committing via an agent/tool, allow a long timeout (10+ minutes) - a
killed hook aborts the commit.

## Rust toolchain is pinned

`rust-toolchain.toml` pins the minor track (currently 1.98). Local dev, hooks
and CI all resolve through it - CI's `@stable` install is only a fallback.
Never rely on a bare `rustup update`d stable locally: a newer clippy adds
lints (e.g. `clippy::question_mark`) that fail CI while passing locally.
Bump the pin deliberately and run `make lint-backend` under it first. The
flatpak sandbox build ignores the pin via `RUSTUP_TOOLCHAIN=stable` (it builds
offline with the SDK's rust).
