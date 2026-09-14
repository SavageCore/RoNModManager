# RoNModManager

## Commit hooks are slow

The lefthook pre-commit hook runs prettier, `cargo fmt --check` and
`cargo clippy`, which can take several minutes (cold cargo builds especially).
When committing via an agent/tool, allow a long timeout (10+ minutes) - a
killed hook aborts the commit.
