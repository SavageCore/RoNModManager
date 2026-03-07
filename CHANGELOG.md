# Changelog

All notable changes to the RoN Mod Manager project are documented in this file.

## [0.1.0] - 2026-03-07

### Added

- **Core Features**
  - Tauri 2 + Svelte 5 desktop application for Ready or Not mod management
  - Cross-platform support (Windows, Linux)
  - Steam game path detection (Windows registry + Linux paths)
  - Modpack sync from HTTP URLs with version comparison
  - Mod.io OAuth authentication
  - Mod installation with archive extraction and file routing
  - Modpack collection management

- **Backend Services** (Phase 2)
  - File hashing (MD5, CRC32) for duplicate detection
  - Steam library path resolution
  - HTTP download with retry logic
  - mod.io API client with rate-limiting
  - Archive extraction with automatic file classification
  - Modpack manifest parsing and download
  - Thread-safe config persistence with atomic writes

- **Command Layer** (Phase 3)
  - Game path detection and configuration
  - Modpack configuration and sync
  - Mod installation/uninstallation
  - Collection toggles
  - mod.io authentication
  - Config management

- **Frontend**
  - Multi-page app shell with navigation sidebar
  - Dashboard with game path detection and modpack sync
  - Mods page with install/uninstall buttons
  - Collections management page
  - Settings page with config, auth, and theme options
  - Progress modal for long-running operations
  - Real-time progress events during mod installation

- **Build & Testing**
  - Comprehensive unit test suite (28 backend tests)
  - Frontend unit tests with Vitest
  - GitHub Actions CI/CD with lint and test verification
  - Multi-platform automated builds (Windows NSIS, Linux AppImage/deb)
  - Automated release workflow with GitHub Releases

### Technical Details

- **Backend**: Rust with Tokio async runtime
- **Frontend**: Svelte 5 + TypeScript + Tailwind CSS 4
- **Architecture**: Tauri IPC for frontend/backend communication
- **Config Storage**: OS-specific app data directories (APPDATA on Windows, XDG_CONFIG_HOME on Linux)
- **File Handling**: Smart archive extraction with CRC32-based deduplication
- **Async I/O**: Progress events via Tauri emit system

## Planned Features

- [ ] HTML fallback for modpack manifest discovery
- [ ] Collection-based installation filtering
- [ ] Modpack export from installed mods
- [ ] Code-based modpack sharing (requires sync server)
- [ ] Auto-updater integration
- [ ] Mod profiles for switching between configurations
- [ ] In-app mod launcher
- [ ] macOS support
