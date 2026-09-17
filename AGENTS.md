# RoNModManager

## Commit hooks are slow

The lefthook pre-commit hook runs prettier, `cargo fmt --check` and
`cargo clippy`, which can take several minutes (cold cargo builds especially).
When committing via an agent/tool, allow a long timeout (10+ minutes) - a
killed hook aborts the commit.

## Commit subjects must be conventional

git-cliff's AppStream config (`packaging/flatpak/cliff-appstream.toml`) and the
release notes config (`cliff.toml`) set `filter_unconventional = true`, so a
subject that is not `type(scope): description` is dropped from `metainfo.xml`
and the release notes - silently, with only a "commit(s) skipped due to parse
error(s)" warning. The lefthook `commit-msg` hook
(`scripts/check-commit-msg.js`) and CI's `lint-commits` job now reject them.

Types: build, chore, ci, docs, feat, fix, perf, refactor, revert, style, test.
Merge commits, `fixup!`/`squash!`, `Revert "..."` and bare version bumps
(`0.0.17`) are exempt. Check a range with
`node scripts/check-commit-msg.js --range v0.0.16..HEAD`.

A squash-merged PR takes its PR title as the commit subject, so keep that
conventional too.

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

One screenshot, once, is the budget. Do not loop: capture, tweak, capture
again, re-read each frame and re-analyse. Confirm the static state you changed,
run the tests, and stop - the user is watching the window and will tell you
what is wrong. Keyboard input does reach the webview (`xdotool key --window
<id> Right` walks the guided tour), so a second capture is available when a
flow genuinely cannot be checked otherwise; it is not a substitute for asking.

Never re-point the dev config's first-run flags (`setup_wizard_complete`,
`tutorial_complete`) or API keys to force a flow back on screen, and never
restart the app to re-enter one. Hot reload mid-flow can flip those flags on
its own - say so and let the user reset or test it.

Leave the dev app running. `make dev` serves through vite with HMR, so source
edits land in the open window on their own - the user watches the change there
and tests it. Do not kill the app or the vite process after starting it, and do
not restart it just to pick up an edit.
