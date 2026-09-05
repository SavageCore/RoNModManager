# RoNModManager

## Typography Standard

NEVER use em-dashes (`—`) anywhere in UI text, code comments, or documentation. Always use a standard hyphen (`-`) instead.

## Commit hooks are slow

The husky pre-commit hook runs prettier, `cargo fmt --check`, `cargo clippy` and
`cargo test`, which can take several minutes (cold cargo builds especially).
When committing via an agent/tool, allow a long timeout (10+ minutes) - a
killed hook aborts the commit and can leave lint-staged's backup unrestored.
