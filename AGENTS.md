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

## Hand UI testing back to the user

Agents cannot drive this app's GUI. Under XWayland, synthetic mouse input
(`xdotool click`, `mousemove`) never reaches the WebKitGTK webview - clicks
produce zero pixel change - and once the docked DevTools pane has focus,
keystrokes stop arriving too. Screenshots and vision checks are fine for
confirming what renders (window id capture via `magick import -window <id>`,
with `GDK_BACKEND=x11`), but no click-through flow can be exercised.

So: cover behaviour with unit tests, use a screenshot to confirm a static
state, then hand the interactive pass to the user and say plainly what was
not exercised. Do not claim a flow works end to end from a build being green.

Leave the dev app running. `make dev` serves through vite with HMR, so source
edits land in the open window on their own - the user watches the change there
and tests it. Do not kill the app or the vite process after starting it, and do
not restart it just to pick up an edit.
