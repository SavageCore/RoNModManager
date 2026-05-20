FLATPAK_ID       := uk.savagecore.ronmodmanager
FLATPAK_MANIFEST := packaging/flatpak/$(FLATPAK_ID).yml
CARGO_MANIFEST   := src-tauri/Cargo.toml

.PHONY: help install dev dev-xwayland build build-frontend release \
        lint format check lint-frontend lint-backend fmt-backend clippy lint-all \
        test-frontend test-backend test \
        flatpak-deps flatpak-vendor flatpak-build flatpak-bundle flatpak-install flatpak-run flatpak \
        clean

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

# ── Dependencies ──────────────────────────────────────────────────────────────

install: ## Install npm dependencies
	npm install

# ── Development ───────────────────────────────────────────────────────────────

dev: ## Run Tauri dev (Wayland-compatible, software rendering)
	WEBKIT_DISABLE_DMABUF_RENDERER=1 LIBGL_ALWAYS_SOFTWARE=1 npm run tauri dev

dev-xwayland: ## Run Tauri dev via XWayland (full window state persistence)
	GDK_BACKEND=x11 npm run tauri dev

# ── Build ─────────────────────────────────────────────────────────────────────

build: ## Build the full Tauri application
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

lint-backend: ## Run cargo fmt check + clippy on the Rust backend
	cargo fmt --manifest-path $(CARGO_MANIFEST) -- --check
	cargo clippy --manifest-path $(CARGO_MANIFEST) --all-targets --all-features -- -D warnings

fmt-backend: ## Auto-format the Rust backend with cargo fmt
	cargo fmt --manifest-path $(CARGO_MANIFEST)

clippy: ## Run cargo clippy on the Rust backend
	cargo clippy --manifest-path $(CARGO_MANIFEST) --all-targets --all-features -- -D warnings

lint-all: lint-frontend lint-backend ## Run all linters (frontend + backend)

format-all: format fmt-backend ## Auto-format all code (frontend + backend)

# ── Test ──────────────────────────────────────────────────────────────────────

test-frontend: ## Run Vitest unit tests
	npm run test:unit

test-backend: ## Run Rust tests with cargo test
	cargo test --manifest-path $(CARGO_MANIFEST)

test: test-frontend test-backend ## Run all tests (frontend + backend)

# ── Flatpak ───────────────────────────────────────────────────────────────────

flatpak-deps: ## Install Flatpak runtimes and SDK extensions (run once)
	flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
	flatpak install -y flathub \
		org.gnome.Platform//50 \
		org.gnome.Sdk//50 \
		org.freedesktop.Sdk.Extension.node24//24.08 \
		org.freedesktop.Sdk.Extension.rust-stable//24.08

flatpak-vendor: ## Vendor Cargo dependencies for offline Flatpak build
	cd src-tauri && cargo vendor vendor

flatpak-build: ## Build Flatpak from local repo (run flatpak-vendor first)
	flatpak-builder --force-clean --user --install-deps-from=flathub --repo=flatpak-repo build-dir $(FLATPAK_MANIFEST)

flatpak-bundle: ## Export a .flatpak bundle from the local repo
	flatpak build-bundle flatpak-repo ronmodmanager.flatpak $(FLATPAK_ID)

flatpak-install: ## Install the local .flatpak bundle for the current user
	flatpak install --user --bundle ronmodmanager.flatpak

flatpak-run: ## Run the installed Flatpak
	flatpak run $(FLATPAK_ID)

flatpak: flatpak-vendor flatpak-build flatpak-bundle ## Full local Flatpak pipeline (vendor → build → bundle)

# ── Clean ─────────────────────────────────────────────────────────────────────

clean: ## Remove build artifacts
	rm -rf build .svelte-kit build-dir flatpak-repo ronmodmanager.flatpak
	cargo clean --manifest-path $(CARGO_MANIFEST)

dev-to-prod: # Copy dev config and mods to production
	python scripts/copy_dev_to_prod.py
