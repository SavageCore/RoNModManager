FLATPAK_ID           := uk.savagecore.ronmodmanager
FLATPAK_MANIFEST     := packaging/flatpak/$(FLATPAK_ID).yml
FLATPAK_GPG_KEY      := 65A3B75AC7807CC569F4730F26970A50720B91A6
FLATPAK_GPG_PUB      := packaging/flatpak/ronmodmanager-flatpak.gpg
FLATPAK_LOCAL_REMOTE := ronmodmanager-local
CARGO_MANIFEST   := src-tauri/Cargo.toml

.PHONY: help install dev dev-xwayland build build-frontend release \
        lint format check lint-frontend lint-backend fmt-backend clippy lint-all \
        test-frontend test-backend test \
        screenshots screenshots-build \
        vendor flatpak-deps update-appstream flatpak-build flatpak-bundle flatpak-install flatpak-run flatpak \
        clean watch

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

# ── Dependencies ──────────────────────────────────────────────────────────────

install: ## Install npm dependencies
	npm install

src-tauri/vendor/.cargo-lock-stamp: src-tauri/Cargo.lock
	cd src-tauri && cargo vendor vendor
	@touch $@

vendor: src-tauri/vendor/.cargo-lock-stamp ## Vendor Cargo dependencies (auto-skipped if Cargo.lock unchanged)

# ── Development ───────────────────────────────────────────────────────────────

dev: vendor ## Run Tauri dev (Wayland-compatible, software rendering)
	WEBKIT_DISABLE_DMABUF_RENDERER=1 LIBGL_ALWAYS_SOFTWARE=1 npm run tauri dev

dev-xwayland: vendor ## Run Tauri dev via XWayland (full window state persistence)
	GDK_BACKEND=x11 npm run tauri dev

watch: dev ## Watch for changes and rebuild Tauri application

# ── Build ─────────────────────────────────────────────────────────────────────

build: vendor ## Build the full Tauri application
	npm run tauri build

build-frontend: ## Build only the Svelte frontend with Vite
	npm run build

release: ## Release build signed via dotenvx (requires .env with signing keys)
	npm run release

# ── Lint & Format ─────────────────────────────────────────────────────────────

lint: ## Check formatting with Prettier (frontend)
	npm run lint

format: ## Auto-format all files with Prettier
	npm run format

check: ## Run svelte-check for TypeScript/Svelte type checking
	npm run check

lint-frontend: lint check ## Run all frontend lint checks (Prettier + svelte-check)

lint-backend: vendor ## Run cargo fmt check + clippy on the Rust backend
	cargo fmt --manifest-path $(CARGO_MANIFEST) -- --check
	cargo clippy --manifest-path $(CARGO_MANIFEST) --all-targets --all-features -- -D warnings

fmt-backend: ## Auto-format the Rust backend with cargo fmt
	cargo fmt --manifest-path $(CARGO_MANIFEST)

clippy: vendor ## Run cargo clippy on the Rust backend
	cargo clippy --manifest-path $(CARGO_MANIFEST) --all-targets --all-features -- -D warnings

lint-all: lint-frontend lint-backend ## Run all linters (frontend + backend)

format-all: format fmt-backend ## Auto-format all code (frontend + backend)

# ── Test ──────────────────────────────────────────────────────────────────────

test-frontend: ## Run Vitest unit tests
	npm run test:unit

test-backend: vendor ## Run Rust tests with cargo test
	cargo test --manifest-path $(CARGO_MANIFEST)

test: test-frontend test-backend ## Run all tests (frontend + backend)

# ── Screenshots ───────────────────────────────────────────────────────────────

screenshots: ## Take light + dark screenshots (rebuild with make screenshots-build if Rust changed)
	SCREENSHOT_THEME=light node scripts/take-screenshots.mjs
	SCREENSHOT_THEME=dark  node scripts/take-screenshots.mjs

screenshots-build: vendor ## Build debug binary then take screenshots (run after Rust changes)
	cargo build --manifest-path $(CARGO_MANIFEST)
	SCREENSHOT_THEME=light node scripts/take-screenshots.mjs
	SCREENSHOT_THEME=dark  node scripts/take-screenshots.mjs

# ── Flatpak ───────────────────────────────────────────────────────────────────

update-appstream: ## Update AppStream releases in metainfo from git history (requires git-cliff)
	git cliff --config packaging/flatpak/cliff-appstream.toml > /tmp/appstream-releases.xml
	python3 -c "\
import re; \
content = open('packaging/flatpak/$(FLATPAK_ID).metainfo.xml').read(); \
releases = open('/tmp/appstream-releases.xml').read().rstrip(); \
open('packaging/flatpak/$(FLATPAK_ID).metainfo.xml', 'w').write(re.sub(r'  <releases>.*?  </releases>', releases, content, flags=re.DOTALL))"

flatpak-deps: ## Install Flatpak runtimes and SDK extensions (run once)
	flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
	flatpak install -y flathub \
		org.gnome.Platform//50 \
		org.gnome.Sdk//50 \
		org.freedesktop.Sdk.Extension.node24//25.08 \
		org.freedesktop.Sdk.Extension.rust-stable//25.08

flatpak-build: ## Build Flatpak from local repo (run vendor first)
	flatpak-builder --force-clean --delete-build-dirs --user --install-deps-from=flathub \
		--gpg-sign=$(FLATPAK_GPG_KEY) \
		--repo=flatpak-repo build-dir $(FLATPAK_MANIFEST)

flatpak-bundle: ## Export a .flatpak bundle from the local repo
	flatpak build-bundle \
		--gpg-sign=$(FLATPAK_GPG_KEY) \
		--gpg-keys=$(FLATPAK_GPG_PUB) \
		flatpak-repo ronmodmanager.flatpak $(FLATPAK_ID) master

flatpak-install: ## Install the locally built Flatpak via a local OSTree remote
	flatpak remote-add --user --if-not-exists --no-gpg-verify \
		$(FLATPAK_LOCAL_REMOTE) "file://$(CURDIR)/flatpak-repo"
	flatpak remote-modify --user --no-gpg-verify \
		--url="file://$(CURDIR)/flatpak-repo" \
		$(FLATPAK_LOCAL_REMOTE)
	flatpak install --user --reinstall -y $(FLATPAK_LOCAL_REMOTE) $(FLATPAK_ID)

flatpak-run: ## Run the installed Flatpak
	flatpak run $(FLATPAK_ID)

flatpak: vendor flatpak-build flatpak-install ## Full local Flatpak pipeline (vendor → build → install)

# ── Clean ─────────────────────────────────────────────────────────────────────

clean: ## Remove build artifacts
	rm -rf build .svelte-kit build-dir flatpak-repo ronmodmanager.flatpak .flatpak-builder/build/
	cargo clean --manifest-path $(CARGO_MANIFEST)

dev-to-prod: # Copy dev config and mods to production
	python scripts/copy_dev_to_prod.py
