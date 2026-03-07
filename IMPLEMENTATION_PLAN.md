# RoN Mod Manager — Implementation Plan

## 1. Project Overview

A cross-platform (Linux + Windows) GUI mod manager for **Ready or Not** (Steam App ID: `1144200`). Replaces the existing Python CLI tool ([RoNModsDownloader](../RoNModsDownloader/)) with a modern, lightweight desktop application.

**Core capabilities carried forward:**

- Download/install mods from **mod.io** API and **self-hosted** modpack servers
- Auto-subscribe users to mod.io mods defined by a modpack
- Intelligent archive extraction: detect `.pak` mods, `.sav` world-gen maps, and game-file overrides
- Modpack collections (toggleable groups of mods)
- Hash-based deduplication (skip unchanged files)

**New/improved:**

- Full GUI instead of CLI/curses — clean, responsive design inspired by [Gale](https://github.com/Kesomannen/gale)
- Light/dark theme (follows system preference, user-overridable)
- **Mod Profiles** — create multiple mod configurations (Vanilla, Tactical Realism, Content Pack, etc.) and switch between them instantly
- **On-demand mod loading** — mods are symlinked/copied only when launching with a profile, preventing conflicts
- **Integrated game launcher** — launch Ready or Not directly from the app with your chosen profile
- **Modpack Builder GUI** — create modpacks from currently installed mods, toggle enabled/disabled per mod, export
- **Share modpacks** via short code (central sync server) or file export (`ronmod.pack` JSON)
- **One-click install links** — `ronmod://` URL scheme for sharing modpacks via clickable links
- Cross-platform (Linux + Windows) from day one
- Proper OS-native config storage (`XDG_CONFIG_HOME` / `%APPDATA%`)
- Automated mod.io API key generation via Tauri webview (Steam OAuth login → token extraction — no Selenium dependency)
- Self-updater built in
- Comprehensive test suite and linting

---

## 2. Technology Stack

### 2.1 Why Tauri v2 + Svelte 5

| Requirement       | Tauri v2 delivers                                              |
| ----------------- | -------------------------------------------------------------- |
| Small binary      | ~10-15 MB (vs 100 MB+ PyInstaller/PySide6, 150 MB+ Electron)   |
| Cross-platform    | Windows, Linux, macOS out of the box                           |
| Linux packaging   | `.deb`, `.rpm`, `.AppImage` built-in; Flatpak via config       |
| Windows packaging | NSIS installer + portable mode                                 |
| Performant        | Rust backend, system webview (no bundled Chromium)             |
| Built-in updater  | `tauri-plugin-updater` with GitHub Releases support            |
| Deep linking      | Custom URL scheme (`ronmod://`) for one-click modpack installs |
| GitHub Actions    | Official `tauri-apps/tauri-action` for CI/CD                   |

**Frontend:** Svelte 5 + TypeScript + Tailwind CSS 4

- Svelte produces the smallest JS bundles of any framework
- Runes reactivity model is simple and performant
- Tailwind 4 for rapid, consistent styling

**Backend (Rust):** All I/O, crypto, archive handling, API calls, file system operations

**Testing:**

- Rust: `cargo test` (unit + integration) with `mockall` for mocking
- Frontend: `vitest` + `@testing-library/svelte`
- E2E: `playwright` (via `@playwright/test`)

**Linting:**

- Rust: `clippy` (deny warnings) + `rustfmt`
- Frontend: `eslint` + `prettier` + `svelte-check`
- Pre-commit: `husky` + `lint-staged`

---

## 3. Project Structure

```
ronmodmanager/
├── .github/
│   ├── workflows/
│   │   ├── ci.yml              # Lint + test on every push/PR
│   │   ├── build.yml           # Build artifacts (Linux + Windows)
│   │   └── release.yml         # Tagged release → GitHub Release + installers
│   └── dependabot.yml
├── src-tauri/                   # Rust backend
│   ├── Cargo.toml
│   ├── tauri.conf.json          # Tauri config (app name, window, permissions, updater)
│   ├── capabilities/
│   │   └── default.json         # Tauri v2 capability permissions
│   ├── icons/                   # App icons (all sizes, auto-generated via `tauri icon`)
│   ├── src/
│   │   ├── main.rs              # Entry point, plugin registration
│   │   ├── lib.rs               # Tauri app builder, command registration
│   │   ├── commands/            # Tauri IPC command handlers (thin layer)
│   │   │   ├── mod.rs
│   │   │   ├── mods.rs          # install/uninstall/list mods
│   │   │   ├── modpack.rs       # fetch/apply/build/export modpack
│   │   │   ├── sharing.rs       # share via code (sync server) + file export/import
│   │   │   ├── auth.rs          # OAuth flow, token status
│   │   │   ├── config.rs        # get/set config values
│   │   │   ├── collections.rs   # toggle collections
│   │   │   ├── profile.rs       # profile CRUD, apply, launch
│   │   │   └── game.rs          # detect game path, get status
│   │   ├── services/            # Business logic (pure Rust, no Tauri deps)
│   │   │   ├── mod.rs
│   │   │   ├── modio_api.rs     # mod.io REST client
│   │   │   ├── modpack.rs       # modpack download, parse & build/export
│   │   │   ├── sync_server.rs   # share-via-code: push/pull to central server
│   │   │   ├── installer.rs     # archive extraction, override logic
│   │   │   ├── hasher.rs        # MD5/CRC32 hashing
│   │   │   ├── steam.rs         # Steam game path detection (cross-platform)
│   │   │   ├── profile.rs       # profile management, mod staging
│   │   │   ├── launcher.rs      # game launch, mod symlink/copy
│   │   │   └── downloader.rs    # HTTP download with progress events
│   │   ├── models/              # Shared data types
│   │   │   ├── mod.rs
│   │   │   ├── config.rs        # AppConfig struct
│   │   │   ├── modpack.rs       # ModPack, Collection, ModSource
│   │   │   ├── modinfo.rs       # ModInfo, ModFile, ModStatus
│   │   │   ├── profile.rs       # Profile, ProfileConfig
│   │   │   └── error.rs         # Error enum, Result type alias
│   │   └── state.rs             # Tauri managed state (AppConfig, reqwest client)
│   └── tests/
│       ├── modio_api_test.rs
│       ├── installer_test.rs
│       ├── steam_test.rs
│       ├── modpack_test.rs
│       ├── sync_server_test.rs
│       └── fixtures/             # Test archives, mock responses
│           ├── test_mod.zip
│           ├── test_override.zip
│           ├── test_worldgen.zip
│           └── mock_responses/
├── src/                          # Svelte frontend
│   ├── app.html
│   ├── app.css                   # Tailwind entry + global styles
│   ├── lib/
│   │   ├── components/
│   │   │   ├── Layout.svelte          # Shell: sidebar + content area
│   │   │   ├── Sidebar.svelte         # Navigation sidebar
│   │   │   ├── ModList.svelte         # Scrollable mod list with status badges
│   │   │   ├── ModCard.svelte         # Individual mod: name, source, status, actions
│   │   │   ├── CollectionPanel.svelte # Collection toggle list
│   │   │   ├── ProgressBar.svelte     # Download/install progress
│   │   │   ├── Toast.svelte           # Notification toasts
│   │   │   ├── SettingsForm.svelte    # Config editing form
│   │   │   ├── AuthStatus.svelte      # OAuth status + login button
│   │   │   ├── GamePathPicker.svelte  # Auto-detect or manual browse
│   │   │   ├── ThemeToggle.svelte     # Light/dark/system theme switcher
│   │   │   ├── ProfileCard.svelte     # Individual profile card with launch button
│   │   │   ├── ProfileEditor.svelte   # Modal: create/edit profile with mod selection
│   │   │   ├── QuickLaunch.svelte     # Dashboard widget: profile dropdown + launch
│   │   │   ├── ModpackBuilder.svelte  # GUI modpack creator (toggle mods, collections)
│   │   │   ├── ShareModal.svelte      # Share via code or export to file
│   │   │   ├── ImportModal.svelte     # Import from code or file
│   │   │   └── DeepLinkDialog.svelte  # Modpack preview + confirm for deep links
│   │   ├── pages/
│   │   │   ├── Dashboard.svelte       # Overview: mod count, sync status, quick actions
│   │   │   ├── Mods.svelte            # Mod list + install/uninstall
│   │   │   ├── Collections.svelte     # Collection toggles
│   │   │   ├── Profiles.svelte        # Profile management page
│   │   │   ├── ModpackExport.svelte   # Modpack builder + share page
│   │   │   └── Settings.svelte        # Config, auth, modpack URL, game path, theme
│   │   ├── stores/
│   │   │   ├── mods.svelte.ts         # Mod state (Svelte 5 runes)
│   │   │   ├── config.svelte.ts       # Config state
│   │   │   ├── profiles.svelte.ts     # Profile state
│   │   │   ├── theme.svelte.ts        # Theme state (light/dark/system)
│   │   │   ├── notifications.svelte.ts
│   │   │   └── progress.svelte.ts     # Download/install progress
│   │   ├── api/
│   │   │   └── commands.ts            # Typed wrappers around Tauri `invoke()` calls
│   │   └── types/
│   │       └── index.ts               # TypeScript interfaces matching Rust models
│   └── main.ts                        # Svelte mount
├── tests/
│   ├── unit/                     # Vitest frontend unit tests
│   │   ├── ModCard.test.ts
│   │   ├── commands.test.ts
│   │   └── stores.test.ts
│   └── e2e/                      # Playwright E2E tests
│       ├── playwright.config.ts
│       ├── install-flow.spec.ts
│       └── settings.spec.ts
├── package.json
├── svelte.config.js
├── vite.config.ts
├── tsconfig.json
├── tailwind.config.ts
├── eslint.config.js
├── .prettierrc
├── .gitignore
├── LICENSE
└── README.md
```

---

## 4. Data Models

### 4.1 Modpack Definition (`ronmod.pack` — JSON)

Renamed from `rmd.pack` to `ronmod.pack`. Keep JSON — it's machine-readable, trivially parseable in Rust, and the modpack author rarely hand-edits it.

```jsonc
{
  "schema_version": 1,
  "name": "SavagePack",
  "version": "1.2.0",
  "description": "SavageCore's Ready or Not Mod Pack",
  "author": "SavageCore",

  // mod.io subscriptions — just the mod name_id (slug) for cleanliness
  "subscriptions": [
    "fairfax-residence-remake",
    "lustful-remorse",
    "hospital-map",
  ],

  // Collections: named groups users can toggle on/off
  "collections": {
    "Beat Cop": {
      "default_enabled": true,
      "description": "Realistic beat cop loadout",
      "mods": [
        "Long Tactical Hider-4453-1-1-1724670588.zip",
        "pakchunk99-Mods_OffHelmet_P.pak",
      ],
    },
    "John Wick": {
      "default_enabled": false,
      "description": "Suit and tie loadout",
      "mods": ["JohnWickSuit.pak"],
    },
  },
}
```

**Server folder structure** (unchanged — NGINX autoindex or any static file host):

```
{modpack_url}/
  ronmod.pack
  mods/
    _collections/{collection_name}/
    _manual/
    _overrides/ReadyOrNot/Content/...
```

### 4.2 App Config (`config.json` — stored in OS app data dir)

**Location:**

- Linux: `$XDG_CONFIG_HOME/ronmodmanager/config.json` (default `~/.config/ronmodmanager/`)
- Windows: `%APPDATA%/ronmodmanager/config.json`

```jsonc
{
  "game_path": "/home/user/.steam/steam/steamapps/common/Ready Or Not",
  "modpack_url": "https://mods.example.com/savagepack",
  "modpack_version": "1.2.0",
  "oauth_token": "encrypted:base64...", // Encrypted at rest (see §5.4)
  "subscribed_mods": {
    "fairfax-residence-remake": {
      "md5": "abc123...",
      "filename": "Fairfax-1234-1-0.zip",
      "download_url": "https://cdn.mod.io/...",
      "contents": ["FairfaxResidence.pak"],
    },
  },
  "collections": {
    "Beat Cop": true,
    "John Wick": false,
  },
  "last_update_check": "2026-03-06T12:00:00Z",
}
```

### 4.3 Rust Models

```rust
// models/config.rs
#[derive(Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub game_path: Option<PathBuf>,
    pub modpack_url: Option<String>,
    pub modpack_version: Option<String>,
    pub oauth_token: Option<String>,
    pub subscribed_mods: HashMap<String, SubscribedMod>,
    pub collections: HashMap<String, bool>,
    pub theme: ThemeMode,
    pub last_update_check: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SubscribedMod {
    pub md5: String,
    pub filename: String,
    pub download_url: String,
    pub contents: Vec<String>,
}

// models/modpack.rs
#[derive(Serialize, Deserialize)]
pub struct ModPack {
    pub schema_version: u32,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    pub subscriptions: Vec<String>,
    pub collections: HashMap<String, Collection>,
}

#[derive(Serialize, Deserialize)]
pub struct Collection {
    pub default_enabled: bool,
    pub description: Option<String>,
    pub mods: Vec<String>,
}

// models/modinfo.rs
#[derive(Serialize, Deserialize, Clone)]
pub struct ModInfo {
    pub name: String,
    pub source: ModSource,
    pub status: ModStatus,
    pub filename: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum ModSource {
    ModIo { mod_id: String },
    ModPack,
    Manual,
    Collection { name: String },
}

#[derive(Serialize, Deserialize, Clone)]
pub enum ModStatus {
    NotInstalled,
    Downloading,
    Downloaded,
    Installed,
    UpdateAvailable,
    Error(String),
}
```

### 4.4 TypeScript Interfaces

```typescript
// src/lib/types/index.ts — mirror Rust models

type ThemeMode = "light" | "dark" | "system";

interface AppConfig {
  game_path: string | null;
  modpack_url: string | null;
  modpack_version: string | null;
  subscribed_mods: Record<string, SubscribedMod>;
  collections: Record<string, boolean>;
  theme: ThemeMode;
  active_profile_id: string | null;
  profiles: Profile[];
  auto_cleanup_on_exit: boolean;
  use_symlinks: boolean;
  last_launch: string | null;
}

interface SubscribedMod {
  md5: string;
  filename: string;
  download_url: string;
  contents: string[];
}

interface ModInfo {
  name: string;
  source: ModSource;
  status: ModStatus;
  filename: string;
}

type ModSource =
  | { type: "mod_io"; mod_id: string }
  | { type: "mod_pack" }
  | { type: "manual" }
  | { type: "collection"; name: string };

type ModStatus =
  | "not_installed"
  | "downloading"
  | "downloaded"
  | "installed"
  | "update_available"
  | { error: string };

interface ProgressEvent {
  task: string;
  current: number;
  total: number;
  message: string;
}

// Modpack sharing
interface ShareCodeResponse {
  code: string; // Short share code (e.g., "ABCD-1234")
  expires_at: string; // ISO datetime, or null for permanent
}

interface SharedModpack {
  modpack: ModPack; // Full modpack definition
  shared_by: string; // Display name of sharer
  created_at: string;
  updated_at: string;
}

// Profiles
interface Profile {
  id: string;
  name: string;
  description: string | null;
  icon: string | null; // Emoji or icon name
  mod_files: string[]; // Filenames in staging area
  collections: Record<string, boolean>;
  created_at: string;
  last_used: string | null;
  is_vanilla: boolean;
  is_default: boolean;
}
```

---

## 5. Backend Implementation Details (Rust)

### 5.1 Steam Game Path Detection (`services/steam.rs`)

Must work on both platforms:

**Windows:**

1. Read registry `HKLM\SOFTWARE\Wow6432Node\Valve\Steam` → `InstallPath`
2. Parse `steamapps/libraryfolders.vdf` (use `keyvalues-serde` crate)
3. Search all library folders for `appmanifest_1144200.acf`
4. Return `{library}/steamapps/common/Ready Or Not`

**Linux:**

1. Check common Steam paths in order:
   - `~/.steam/steam/steamapps/`
   - `~/.local/share/Steam/steamapps/`
   - Flatpak: `~/.var/app/com.valvesoftware.Steam/.steam/steam/steamapps/`
   - Snap: `~/snap/steam/common/.steam/steam/steamapps/`
2. Parse `libraryfolders.vdf` same as Windows
3. Search for `appmanifest_1144200.acf`

**Game paths derived:**

- Mods: `{game_path}/ReadyOrNot/Content/Paks/~mods`
- SaveGames (Windows): `%LOCALAPPDATA%/ReadyOrNot/Saved/SaveGames`
- SaveGames (Linux/Proton): `{steam_path}/steamapps/compatdata/1144200/pfx/drive_c/users/steamuser/AppData/Local/ReadyOrNot/Saved/SaveGames`

**Crate:** `winreg` (Windows-only, behind `#[cfg(target_os = "windows")]`)

### 5.2 mod.io API Client (`services/modio_api.rs`)

Use `reqwest` with a shared `Client` (connection pooling, managed as Tauri state).

**Base URL:** `https://api.mod.io/v1`
**Game ID:** `3791` (Ready or Not)
**Auth:** `Authorization: Bearer {oauth_token}` header

**Endpoints to implement:**

| Method | Endpoint                              | Purpose               |
| ------ | ------------------------------------- | --------------------- |
| GET    | `/me/subscribed?game_id=3791`         | Fetch subscribed mods |
| POST   | `/games/3791/mods/{mod_id}/subscribe` | Subscribe to mod      |
| DELETE | `/games/3791/mods/{mod_id}/subscribe` | Unsubscribe           |
| GET    | `/games/3791/mods?name_id={slug}`     | Resolve slug → mod_id |

**Error handling:**

- 401 → Mark token invalid, emit event to frontend to re-auth
- 403 → Skip mod (hidden/DMCA), log warning
- 429 → Respect `Retry-After` header
- Network errors → Retry with exponential backoff (max 3 attempts)

**Slug resolution:** The modpack lists mods by `name_id` (e.g., `"fairfax-residence-remake"`). The subscribe endpoint needs the numeric mod ID. Use the search endpoint to resolve: `GET /games/3791/mods?name_id={slug}` → extract `data[0].id`.

### 5.3 mod.io Authentication via Tauri Webview

mod.io accounts are linked via **Steam OAuth** — users log in to mod.io using their Steam account. After login, an **OAuth2 Access Token** (API key) must be created at `https://mod.io/me/access`. This token is required for all API calls (subscribe, unsubscribe, download).

The original CLI app used Selenium + ChromeDriver to automate this. The new app replaces that with a **Tauri webview window** — same UX, zero external dependencies.

#### Flow: Automated Token Generation

1. User clicks **"Login to mod.io"** in Settings
2. App opens a **secondary Tauri webview window** pointing to `https://mod.io` login page
3. User logs in via **Steam OAuth** (mod.io's standard login mechanism)
4. Once authenticated (detected by monitoring URL changes for successful login), the webview navigates to `https://mod.io/me/access`
5. App injects JavaScript into the webview to:
   - Click "Create Token" (or equivalent UI action)
   - Set token name to `"RoN Mod Manager"`
   - Extract the generated OAuth2 access token from the page DOM
6. Token is captured, stored securely (see §5.4), and the webview window closes
7. Backend validates the token with a test API call (`GET /me/subscribed?game_id=3791`)

#### Implementation Details

```rust
// commands/auth.rs
use tauri::{WebviewWindowBuilder, WebviewUrl};

#[tauri::command]
async fn open_modio_login(app: AppHandle) -> Result<()> {
    // Open secondary webview window for mod.io login
    WebviewWindowBuilder::new(
        &app,
        "modio-auth",
        WebviewUrl::External("https://mod.io".parse().unwrap()),
    )
    .title("Login to mod.io")
    .inner_size(1024.0, 768.0)
    .build()?;
    Ok(())
}
```

Use Tauri's `on_navigation` or `on_page_load` event handlers to detect when the user reaches `/me/access`, then inject JS to extract the token.

#### Fallback: Manual Token Paste

If webview automation fails (mod.io changes their page structure), provide a fallback:

1. Open `https://mod.io/me/access` in the **system browser** (via `tauri::shell::open`)
2. Show a text field in Settings: "Paste your OAuth2 Access Token here"
3. User creates the token manually on mod.io and pastes it

Both paths end at the same place: token stored in keychain, validated via API call.

### 5.4 Token Storage

Store the OAuth token encrypted at rest using `tauri-plugin-store` or native OS keychain:

**Primary:** Use OS keychain via the `keyring` crate:

- Linux: Secret Service (GNOME Keyring / KWallet)
- Windows: Windows Credential Manager

**Fallback:** If keychain unavailable (e.g., headless Linux), store in config file with a warning in the UI.

### 5.5 Archive Extraction & Mod Detection (`services/installer.rs`)

Use the `zip` crate for extraction. Port the existing detection logic:

```rust
pub enum ModFileType {
    PakMod,          // .pak → install to ~mods/
    WorldGenSave,    // .sav → install to SaveGames/
    Override,        // mirrors game directory structure
    Unknown,         // skip
}

pub fn classify_archive_entry(path: &Path) -> ModFileType {
    match path.extension().and_then(|e| e.to_str()) {
        Some("pak") => ModFileType::PakMod,
        Some("sav") => ModFileType::WorldGenSave,
        _ => ModFileType::Unknown,
    }
}
```

**Override detection:** Files under `_overrides/` are copied into the game directory, with originals backed up to `{game_path}/.ronmod_backups/`. Backup filenames include a hash to support nested paths.

**Extraction rules (same as original):**

1. `.pak` at any depth → copy to `~mods/` (flatten)
2. `.sav` → copy to SaveGames directory
3. Files in `_overrides/` → mirror directory structure into game root, backup originals
4. All other files → skip

**Hash-based skip logic:**

- Before extracting, compare CRC32 of zip entry vs existing file (if any)
- Skip if identical — avoids unnecessary disk I/O

### 5.6 Self-Hosted Modpack Download (`services/modpack.rs`)

Port the NGINX directory listing parser:

1. Fetch `{modpack_url}/ronmod.pack` → parse JSON → `ModPack` struct
2. Compare `modpack.version` vs `config.modpack_version` using `semver` crate
3. If newer, download mod files from `{modpack_url}/mods/` subdirectories
4. Parse HTML directory listings to discover files (use `scraper` crate for HTML parsing)
5. Download files with progress events emitted to frontend
6. Skip files that already exist locally (by filename — existing behaviour)

**Improvement:** Add optional `ronmod.manifest` file alongside `ronmod.pack`:

```jsonc
{
  "files": [
    { "path": "_manual/CustomMod.pak", "sha256": "abc123...", "size": 1048576 },
    {
      "path": "_collections/Beat Cop/Hider.zip",
      "sha256": "def456...",
      "size": 2097152,
    },
  ],
}
```

If this manifest exists, use it instead of crawling HTML — faster, reliable, hash-verified. If it doesn't exist, fall back to HTML directory listing parsing. Provide a small CLI tool or script (`generate-manifest.py`) that modpack hosts can run to auto-generate the manifest.

### 5.7 File Downloads with Progress (`services/downloader.rs`)

```rust
pub async fn download_file(
    client: &Client,
    url: &str,
    dest: &Path,
    app_handle: &AppHandle,
    task_label: &str,
) -> Result<()> {
    let response = client.get(url).send().await?;
    let total = response.content_length().unwrap_or(0);
    let mut stream = response.bytes_stream();
    let mut file = tokio::fs::File::create(dest).await?;
    let mut downloaded: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        app_handle.emit("download-progress", ProgressEvent {
            task: task_label.to_string(),
            current: downloaded,
            total,
            message: format!("Downloading {task_label}"),
        })?;
    }
    Ok(())
}
```

Emit Tauri events so the frontend can show real-time progress bars.

### 5.8 Modpack Sharing (`services/sync_server.rs`)

Two sharing methods, inspired by Gale's profile sharing:

#### Option A: Share via File

Simplest path — no server needed:

1. User builds a modpack in the GUI (see §6.6)
2. Clicks **"Export to File"** → writes `ronmod.pack` JSON via Tauri save dialog
3. Recipient clicks **"Import from File"** → loads the JSON, app subscribes to listed mods + downloads modpack files

This works today with the self-hosted server workflow — the file is the same `ronmod.pack` format.

#### Option B: Share via Code (central sync server)

For frictionless sharing — like Gale's profile sync. Requires a lightweight central server.

**Flow:**

```
Owner:                              Server:                         Subscriber:
1. Build modpack in GUI             │                                │
2. Click "Share via Code"           │                                │
3. POST /modpacks → ───────────→ Store modpack JSON               │
4. Receive code: ABCD-1234    ←───  Return share code               │
5. Share code with team             │                                │
   │                                │                    6. Enter code: ABCD-1234
   │                                │ ←───────────────  7. GET /modpacks/{code}
   │                                │ ───────────────→  8. Receive modpack JSON
   │                                │                    9. App applies modpack
   │                                │                                │
10. Update mods, click "Push"       │                                │
11. PUT /modpacks/{code}   ─────→  Update stored JSON              │
    │                               │                    12. App checks for updates
    │                               │ ←───────────────  13. GET /modpacks/{code}
    │                               │ ───────────────→  14. Updated modpack JSON
```

**Sync Server (separate repo: `ronmod-sync`):**

Minimal REST API — can be a tiny Rust binary (Axum) or Cloudflare Worker:

| Method | Endpoint | Purpose |
|--------|----------|---------||
| POST | `/modpacks` | Create shared modpack, returns `{ code }` |
| GET | `/modpacks/{code}` | Fetch modpack by code |
| PUT | `/modpacks/{code}` | Update modpack (owner auth via bearer token) |
| DELETE | `/modpacks/{code}` | Delete modpack (owner auth) |
| GET | `/modpacks/{code}/version` | Lightweight version check (returns `{ version, updated_at }`) |

**Share codes:** 8-character alphanumeric codes (e.g., `ABCD-1234`), generated with `nanoid`.

**Auth model:**

- **Creating** a modpack returns a `owner_token` (random UUID) — stored locally by the creator
- **Updating/deleting** requires the `owner_token` in `Authorization` header
- **Reading** is public (anyone with the code can import)
- No user accounts needed — ownership is by token possession

**Storage:** SQLite (single file) or a managed Postgres. Modpacks are small JSON blobs (~1–10 KB each).

**Expiry:** Modpacks not accessed in 90 days are auto-deleted. Owner can set permanent.

**Implementation priority:** Share via file first (Phase 5). Share via code is Phase 6 (requires server deployment). The app should work fully without the sync server.

### 5.8 Mod Lifecycle Commands (`commands/`)

These are the Tauri IPC commands the frontend invokes:

```rust
// commands/game.rs
#[tauri::command]
async fn detect_game_path(state: State<'_, AppState>) -> Result<Option<String>>;

#[tauri::command]
async fn set_game_path(path: String, state: State<'_, AppState>) -> Result<()>;

// commands/auth.rs
#[tauri::command]
async fn get_auth_status(state: State<'_, AppState>) -> Result<bool>;

#[tauri::command]
async fn open_modio_login(app: AppHandle) -> Result<()>;

#[tauri::command]
async fn save_token(token: String, state: State<'_, AppState>) -> Result<()>;

#[tauri::command]
async fn validate_token(state: State<'_, AppState>) -> Result<bool>;

#[tauri::command]
async fn logout(state: State<'_, AppState>) -> Result<()>;

// commands/modpack.rs
#[tauri::command]
async fn set_modpack_url(url: String, state: State<'_, AppState>) -> Result<()>;

#[tauri::command]
async fn sync_modpack(app: AppHandle, state: State<'_, AppState>) -> Result<ModPack>;

#[tauri::command]
async fn build_modpack_from_installed(state: State<'_, AppState>) -> Result<ModPack>;

#[tauri::command]
async fn export_modpack_to_file(modpack: ModPack, path: String) -> Result<()>;

// commands/sharing.rs
#[tauri::command]
async fn share_modpack_via_code(
    modpack: ModPack,
    state: State<'_, AppState>,
) -> Result<String>;  // Returns share code

#[tauri::command]
async fn import_from_code(
    code: String,
    state: State<'_, AppState>,
) -> Result<ModPack>;

#[tauri::command]
async fn push_modpack_update(
    code: String,
    modpack: ModPack,
    state: State<'_, AppState>,
) -> Result<()>;

#[tauri::command]
async fn import_modpack_from_file(path: String) -> Result<ModPack>;

// commands/mods.rs
#[tauri::command]
async fn get_mod_list(state: State<'_, AppState>) -> Result<Vec<ModInfo>>;

#[tauri::command]
async fn install_mods(app: AppHandle, state: State<'_, AppState>) -> Result<()>;

#[tauri::command]
async fn uninstall_mods(app: AppHandle, state: State<'_, AppState>) -> Result<()>;

// commands/collections.rs
#[tauri::command]
async fn get_collections(state: State<'_, AppState>) -> Result<HashMap<String, CollectionInfo>>;

#[tauri::command]
async fn toggle_collection(name: String, enabled: bool, state: State<'_, AppState>) -> Result<()>;

// commands/config.rs
#[tauri::command]
async fn get_config(state: State<'_, AppState>) -> Result<AppConfig>;
```

---

## 6. Frontend Implementation Details (Svelte 5)

### 6.0 UI Design Reference

**Primary inspiration:** [Gale mod manager](https://github.com/Kesomannen/gale) — same stack (Tauri + Svelte), clean aesthetic.

**Key design principles from Gale to follow:**

- **Clean sidebar navigation** with icon + label, active state highlight
- **Card-based mod list** — each mod is a row with name, version/status badge, action buttons on hover
- **Minimal chrome** — no excessive borders or decoration, content-first
- **Responsive** — panels adjust to window size gracefully
- **Modal dialogs** for focused flows (export, import, auth)
- **Muted color palette** with accent color for actions/CTAs

**Iconography:** Use [Lucide icons](https://lucide.dev/) (MIT, tree-shakeable SVG icons) or [Material Symbols](https://fonts.google.com/icons) as Gale does.

### 6.1 Theming (Light / Dark / System)

**Implementation:** Tailwind CSS 4’s built-in dark mode via `class` strategy.

**How it works:**

1. `theme.svelte.ts` store holds current `ThemeMode` (`"light"` | `"dark"` | `"system"`)
2. On app startup, read `config.theme` and apply:
   - `"system"` → use `window.matchMedia("(prefers-color-scheme: dark)")` + listen for changes
   - `"light"` / `"dark"` → set directly
3. Toggle `dark` class on `<html>` element
4. All Tailwind classes use `dark:` variant for dark mode styles
5. Persist choice to config on change

**Theme toggle widget:** `ThemeToggle.svelte` — three-way toggle (sun / monitor / moon icons) in the sidebar footer or settings page.

**Tailwind CSS tokens (example):**

```css
/* app.css */
:root {
  --color-bg: theme(colors.zinc.50);
  --color-surface: theme(colors.white);
  --color-text: theme(colors.zinc.900);
  --color-accent: theme(colors.blue.600);
}
.dark {
  --color-bg: theme(colors.zinc.900);
  --color-surface: theme(colors.zinc.800);
  --color-text: theme(colors.zinc.100);
  --color-accent: theme(colors.blue.400);
}
```

### 6.2 Page Structure

**Single-window app with sidebar navigation (Gale-style):**

```
┌─────────────────────────────────────────────┐
│  RoN Mod Manager                     [─][□][×] │
├──────────┬──────────────────────────────────┤
│ 🏠       │                                  │
│ Dashboard│  Dashboard                       │
│ 📦       │  ┌──────────┐ ┌──────────┐      │
│ Mods     │  │ 12 Mods  │ │ Synced ✓ │      │
│ 📁       │  │ Installed │ │ v1.2.0   │      │
│ Collect. │  └──────────┘ └──────────┘      │
│ 📤       │                                  │
│ Export   │  [Sync Modpack]  [Install All]   │
│ ⚙️       │                                  │
│ Settings │  Recent Activity                 │
│          │  • Downloaded Fairfax Remake     │
│──────────│  • Installed 3 mods              │
│ ☀️ ◑ 🌙  │                                  │
├──────────┴──────────────────────────────────┤
│  ████████░░░░░  Downloading... 65%          │
└─────────────────────────────────────────────┘
```

Note: Theme toggle (sun/monitor/moon) lives in sidebar footer.

### 6.3 Dashboard Page

- Mod count cards (installed / available / updates)
- Modpack sync status + version
- Quick action buttons: "Sync Modpack", "Install All", "Uninstall All"
- Activity log (last 10 operations)

### 6.4 Mods Page

- Scrollable list of all mods (subscriptions + manual + collection mods)
- Each mod card shows: name, source icon (mod.io / modpack / manual), status badge
- Bulk actions: Install All, Uninstall All
- Search/filter bar

### 6.5 Collections Page

- List all collections from active modpack
- Toggle switch per collection (enabled/disabled)
- Shows which mods each collection contains (expandable)
- Changes saved immediately to config

### 6.6 Modpack Builder & Export Page

- **Game Path:** Auto-detected path with "Browse" button override (use Tauri file dialog)
- **Modpack URL:** Text input with validation (tries to fetch `ronmod.pack`)
- **mod.io Auth Flow diagram:**
  ```
  [Login to mod.io] →  Webview opens mod.io  →  User clicks "Steam Login"
       ↓                                              ↓
  Token stored     ←  JS extracts token  ←  Webview navigates to /me/access
       ↓
  [Status: Authenticated ✓]    (or fallback: manual paste)
  ```
- **mod.io Auth:** Status indicator + "Login to mod.io" button → opens webview for Steam OAuth login + automatic token generation. Manual token paste fallback
- **Theme:** Three-way toggle — Light / System / Dark
- **About:** App version, links, update check button

### 6.8 Progress & Notifications

- Global progress bar at bottom of window for downloads/installs
- Listen to Tauri events (`download-progress`, `install-progress`) via `listen()`
- Toast notifications for success/error states using a store-driven toast queue

### 6.9 Frontend-Backend Communication

All calls go through typed wrappers in `src/lib/api/commands.ts`:

```typescript
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AppConfig, ModInfo, ProgressEvent } from "../types";

export const getConfig = () => invoke<AppConfig>("get_config");
export const detectGamePath = () => invoke<string | null>("detect_game_path");
export const setGamePath = (path: string) => invoke("set_game_path", { path });
export const installMods = () => invoke("install_mods");
export const getModList = () => invoke<ModInfo[]>("get_mod_list");
// ... etc

export const onProgress = (cb: (e: ProgressEvent) => void) =>
  listen<ProgressEvent>("download-progress", (event) => cb(event.payload));
```

---

## 7. CI/CD & Packaging

### 7.1 GitHub Actions Workflows

#### `ci.yml` — Lint + Test (every push & PR)

```yaml
jobs:
  lint-frontend:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: 22 }
      - run: npm ci
      - run: npm run lint
      - run: npm run check # svelte-check
      - run: npm run test:unit # vitest

  lint-backend:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: clippy, rustfmt }
      - run: cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
      - run: cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
      - run: cargo test --manifest-path src-tauri/Cargo.toml
```

#### `build.yml` — Build Artifacts (pushes to main)

```yaml
jobs:
  build:
    strategy:
      matrix:
        include:
          - os: ubuntu-22.04
            target: linux
          - os: windows-latest
            target: windows
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: tauri-apps/tauri-action@v0
        with:
          tauriScript: npx tauri
      - uses: actions/upload-artifact@v4
```

#### `release.yml` — Tagged Release

```yaml
on:
  push:
    tags: ["v*"]
jobs:
  release:
    strategy:
      matrix:
        include:
          - os: ubuntu-22.04
          - os: windows-latest
    runs-on: ${{ matrix.os }}
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          tagName: ${{ github.ref_name }}
          releaseName: "v__VERSION__"
          releaseBody: "See CHANGELOG.md for details."
          tauriScript: npx tauri
```

### 7.2 Linux Package Outputs

Tauri natively produces:

- `.deb` (Debian/Ubuntu)
- `.rpm` (Fedora/RHEL) — via `tauri.conf.json` → `bundle.targets`
- `.AppImage` — via `tauri.conf.json` → `bundle.targets`

**Flatpak:** Requires a separate `com.github.savagecore.ronmodmanager.yml` manifest. Add a `flatpak` job to `release.yml` that builds the Flatpak bundle from the AppImage or from source. Use `flatpak-builder` in CI.

### 7.3 Windows Package Outputs

Tauri natively produces:

- `.msi` (WiX installer)
- `.exe` (NSIS installer — more customizable, recommended)

Both install to `%LOCALAPPDATA%/Programs/RoN Mod Manager/` and create Start Menu entries.

### 7.4 Auto-Updater

Use `tauri-plugin-updater`:

```json
// tauri.conf.json
{
  "plugins": {
    "updater": {
      "endpoints": [
        "https://github.com/SavageCore/RoNModManager/releases/latest/download/latest.json"
      ],
      "pubkey": "dW50cnVzdGVkIGNvbW1lbnQ..."
    }
  }
}
```

`tauri-action` automatically generates `latest.json` with signatures on each release. The app checks on startup (rate-limited to once per hour) and prompts the user to update.

---

## 8. Testing Strategy

### 8.1 Rust Unit Tests (target: >90% coverage of `services/`)

**`services/modio_api.rs`:**

- Mock HTTP responses with `mockito` or `wiremock`
- Test successful subscription fetch, 401 handling, 403 skip, 429 retry
- Test slug → mod_id resolution

**`services/installer.rs`:**

- Create test zip archives in `fixtures/`
- Test `.pak` extraction → correct destination
- Test `.sav` extraction → SaveGames path
- Test override installation + backup creation
- Test hash-based skip logic (CRC32 match → no extract)
- Test nested archive structures

**`services/steam.rs`:**

- Mock registry reads (Windows) / mock filesystem paths (Linux)
- Test `libraryfolders.vdf` parsing with real sample files
- Test game not found → `None`
- Test multiple library folders

**`services/modpack.rs`:**

- Test `ronmod.pack` parsing (valid, missing fields, schema mismatch)
- Test version comparison
- Test HTML directory listing parsing with sample HTML
- Test manifest-based file list

**`services/hasher.rs`:**

- Test MD5 against known hashes
- Test CRC32 against known values
- Test large file hashing (stream-based, not load-all-into-memory)

**`services/downloader.rs`:**

- Mock HTTP server, verify file written correctly
- Test resume on partial download (if implemented)
- Test progress event emission

**`services/sync_server.rs`:**

- Mock sync server responses
- Test create modpack → receive code
- Test fetch modpack by code
- Test update modpack (valid owner token vs rejected)
- Test version check endpoint

### 8.2 Rust Integration Tests

- Full workflow: parse modpack → resolve subscriptions → download → extract → verify installed files
- Uses temp directories (`tempfile` crate) for isolation

### 8.3 Frontend Unit Tests (Vitest)

- Test Svelte components render correctly with mock data
- Test store reactivity (mod list updates, progress changes)
- Test command wrapper functions (mock `invoke`)

### 8.4 E2E Tests (Playwright)

- Test full app flow: set game path → set modpack URL → sync → install → verify UI state
- Test settings persistence across app restarts
- Test error states (invalid URL, auth failure)

### 8.5 Coverage Tooling

- Rust: `cargo-llvm-cov` in CI, report uploaded to Codecov
- Frontend: `vitest --coverage` (v8 provider), report uploaded to Codecov
- Minimum thresholds enforced in CI: 80% line coverage

---

## 9. Key Rust Crates

| Crate                            | Purpose                            |
| -------------------------------- | ---------------------------------- |
| `tauri` v2                       | App framework                      |
| `reqwest`                        | HTTP client (async, TLS)           |
| `serde` / `serde_json`           | Serialization                      |
| `tokio`                          | Async runtime (bundled with Tauri) |
| `zip`                            | Archive extraction                 |
| `semver`                         | Version comparison                 |
| `scraper`                        | HTML parsing (NGINX dir listings)  |
| `keyvalues-serde`                | Steam VDF file parsing             |
| `md-5` / `crc32fast`             | Hashing                            |
| `keyring`                        | OS keychain access                 |
| `chrono`                         | Datetime handling                  |
| `thiserror`                      | Error type derivation              |
| `tracing` / `tracing-subscriber` | Structured logging                 |
| `tempfile`                       | Test temp directories              |
| `mockito`                        | HTTP mocking in tests              |
| `mockall`                        | Trait mocking                      |
| `nanoid`                         | Share code generation              |
| `uuid`                           | Owner token generation             |

---

## 10. Implementation Order

Implement in this order. Each phase should be a working, testable increment. **Commit after each numbered step.**

### Phase 1: Project Scaffold

1. `git init`, create `.gitignore`, `LICENSE` (MIT), `README.md`
2. `npm create tauri-app@latest` — Svelte 5, TypeScript
3. Install Tailwind CSS 4 (`@tailwindcss/vite`)
4. Configure ESLint, Prettier, `svelte-check`
5. Configure `clippy`, `rustfmt` in `src-tauri/`
6. Set up `husky` + `lint-staged` pre-commit hooks
7. Create CI workflow (`ci.yml`) — lint + test (both languages)
8. Verify `cargo tauri dev` starts successfully

### Phase 2: Core Rust Services (no UI yet)

9. Implement `models/` — all structs, enums, error types
10. Implement `services/hasher.rs` + tests
11. Implement `services/steam.rs` + tests (both platforms)
12. Implement `services/modio_api.rs` — subscription fetch + subscribe/unsubscribe + tests
13. Implement `services/downloader.rs` — file download with progress events + tests
14. Implement `services/installer.rs` — archive detection + extraction + tests
15. Implement `services/modpack.rs` — parse + download + HTML listing + tests
16. Implement `state.rs` — managed state (config + HTTP client)
17. Implement config load/save to OS app data dir

### Phase 3: Tauri Commands (bridge layer)

18. Implement `commands/config.rs` — get/set config
19. Implement `commands/game.rs` — detect/set game path
20. Implement `commands/auth.rs` — webview login flow + manual token fallback
21. Implement `commands/mods.rs` — list/install/uninstall
22. Implement `commands/modpack.rs` — sync modpack
23. Implement `commands/collections.rs` — get/toggle
24. Register all commands in `lib.rs`

### Phase 4: Frontend UI

25. Build `Layout.svelte` + `Sidebar.svelte` with Tailwind (Gale-style sidebar with icons)
26. Implement `ThemeToggle.svelte` + `theme.svelte.ts` — light/dark/system theming
27. Build `Settings.svelte` — game path detection + modpack URL + auth flow + theme toggle
28. Build `Dashboard.svelte` — status cards + quick actions
29. Build `Mods.svelte` + `ModCard.svelte` — mod list with status
30. Build `Collections.svelte` + `CollectionPanel.svelte`
31. Build `ProgressBar.svelte` + progress event listener
32. Build `Toast.svelte` + notification store
33. Wire all pages to Tauri commands via `api/commands.ts`

### Phase 4.5: Profiles & Game Launcher

34. Implement `models/profile.rs` — Profile struct and ProfileConfig
35. Implement `services/profile.rs` — profile CRUD logic + mod staging
36. Implement `services/launcher.rs` — game launch + mod symlinking/copying
37. Implement `commands/profile.rs` — all profile commands (CRUD + launch)
38. Build `Profiles.svelte` page — profile cards with launch buttons
39. Build `ProfileEditor.svelte` modal — create/edit profiles with mod selection
40. Build `QuickLaunch.svelte` dashboard widget — profile dropdown + launch button
41. Add profile selector to sidebar + integrate launch functionality
42. Implement desktop shortcut creation for profiles (Windows .lnk + Linux .desktop)
43. Add `ronmod://launch?profile=X` deep link handler (extend existing handler)
44. Test symlink creation on both platforms (check Windows Developer Mode requirement)
45. Test game launch via Steam URL on both platforms
46. Test switching between profiles (Vanilla → Modded → Different Modded → Vanilla)

### Phase 5: Modpack Builder & File Sharing

47. Implement `services/modpack.rs` — `build_modpack_from_installed()` logic
48. Implement `commands/modpack.rs` — `build_modpack_from_installed` + `export_modpack_to_file` commands
49. Implement `commands/sharing.rs` — `import_modpack_from_file` command
50. Build `ModpackBuilder.svelte` — GUI for creating modpacks from installed mods
51. Build `ModpackExport.svelte` page — metadata form + export/share actions
52. Build `ImportModal.svelte` + `ShareModal.svelte` — import from file, display share result
53. Implement deep linking: register `ronmod://` URL scheme in `tauri.conf.json`
54. Implement deep link handler (single-instance plugin + event emission)
55. Implement `get_modpack_metadata` command
56. Build `DeepLinkDialog.svelte` — modpack preview + confirm/cancel
57. Wire deep link handler in `main.ts` + add "Copy Install Link" to Share Modal
58. Test deep linking on Linux and Windows
59. Add "Create Profile from Modpack" option in import flow

### Phase 6: Share via Code (sync server)

60. Create `ronmod-sync` repo — minimal Axum REST API (separate project)
61. Implement sync server: POST/GET/PUT/DELETE modpacks endpoints
62. Deploy sync server (Fly.io / Railway / Cloudflare Worker)
63. Implement `services/sync_server.rs` — client for sync API + tests
64. Implement `commands/sharing.rs` — `share_modpack_via_code`, `import_from_code`, `push_modpack_update`
65. Wire share-via-code flow in `ShareModal.svelte` + `ImportModal.svelte`
66. Add `ronmod://import?code=XXX` deep link support for share codes

### Phase 7: Integration & Polish

67. End-to-end testing with Playwright (including profile switching and game launch)
68. OS keychain integration for token storage
69. Auto-updater (`tauri-plugin-updater`) configuration
70. App icon generation (`cargo tauri icon`)
71. Build workflow (`build.yml`) — Linux + Windows artifacts
72. Release workflow (`release.yml`) — tagged releases
73. Flatpak manifest + build job
74. Final README with screenshots, install instructions, usage guide, profile system docs

---

## 11. Development Commands Reference

```bash
# Dev
npm run tauri dev                    # Start app in dev mode (hot reload)

# Frontend
npm run dev                          # Vite dev server only
npm run build                        # Build frontend
npm run lint                         # ESLint
npm run format                       # Prettier
npm run check                        # svelte-check (type checking)
npm run test:unit                    # Vitest
npm run test:e2e                     # Playwright

# Backend
cd src-tauri
cargo fmt                            # Format Rust
cargo clippy -- -D warnings          # Lint Rust
cargo test                           # Run Rust tests
cargo llvm-cov                       # Coverage report

# Build
npm run tauri build                  # Production build (current platform)
npm run tauri build -- --target x86_64-unknown-linux-gnu   # Cross-compile
npm run tauri icon src-tauri/icons/app-icon.png             # Generate all icon sizes
```

---

## 12. Conventions & Code Quality Rules

1. **DRY:** Extract shared logic into `services/`. Commands are thin wrappers that call services.
2. **Error handling:** All errors flow through `models/error.rs` via `thiserror`. Commands return `Result<T, String>` (Tauri requirement) — convert at the command boundary only.
3. **No `unwrap()` in production code.** Use `?` operator everywhere. `unwrap()` is permitted only in tests.
4. **Logging:** Use `tracing` macros (`info!`, `warn!`, `error!`) — never `println!`. The frontend receives errors via command return values and events.
5. **Config mutations:** Always go through a central `save_config()` function that serializes + writes atomically (write to `.tmp` then rename).
6. **Frontend state:** Use Svelte 5 runes (`$state`, `$derived`) in `.svelte.ts` files. No legacy stores.
7. **Component size:** Keep components under ~150 lines. Extract sub-components when they grow.
8. **Commit messages:** Conventional Commits format (`feat:`, `fix:`, `test:`, `chore:`, `docs:`).
9. **Branch strategy:** Work on `main`. Each phase can optionally be a feature branch merged via PR.
10. **Git commits:** Commit after every completed step in the implementation order (§10). Atomic commits, not mega-commits.

---

## 13. Deep Linking (URL Handler)

### 13.1 Overview

Implement a custom URL scheme (`ronmod://`) for one-click modpack installation. Users can share links that automatically open the app and prompt to install a modpack, eliminating the need to manually copy/paste URLs in settings.

**Example user flow:**

1. User clicks link: `ronmod://install?url=https://mods.example.com/savagepack`
2. App opens (or comes to foreground if already running)
3. App shows dialog: "Install modpack from `https://mods.example.com/savagepack`?"
   - Shows modpack name, version, author (fetched from `ronmod.pack`)
   - Preview: "12 mods, 3 collections"
4. User clicks "Install" → app sets modpack URL in config, syncs, and installs
5. User clicks "Cancel" → dialog closes, no changes made

### 13.2 Supported URL Schemes

**Primary: Direct modpack URL**

```
ronmod://install?url=https://mods.example.com/savagepack
```

- Downloads and parses `ronmod.pack` from the provided URL
- Shows preview dialog with modpack metadata
- On confirm: sets as active modpack, syncs, and installs

**Alternative: Share code (requires sync server)**

```
ronmod://import?code=ABCD-1234
```

- Fetches modpack from central sync server by code
- Shows same preview dialog
- On confirm: imports and installs modpack

**Optional: mod.io subscription link** (future enhancement)

```
ronmod://subscribe?mod=fairfax-residence-remake
```

- Subscribes to a single mod.io mod by slug
- Useful for sharing individual mods

### 13.3 Implementation

#### Tauri Configuration

Register the custom URL scheme in `tauri.conf.json`:

```json
{
  "bundle": {
    "linux": {
      "desktop": {
        "mimeType": ["x-scheme-handler/ronmod"]
      }
    },
    "windows": {
      "webviewInstallMode": {
        "type": "embedBootstrapper"
      }
    }
  },
  "deepLinks": {
    "protocol": "ronmod"
  }
}
```

**Linux:** Creates a `.desktop` file entry that registers `x-scheme-handler/ronmod`

**Windows:** Registers the protocol in the Windows Registry during installation via the NSIS/MSI installer

#### Backend Handler

```rust
// main.rs
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            // Register deep link handler
            let handle = app.handle().clone();
            tauri_plugin_deep_link::register("ronmod", move |request| {
                handle.emit("deep-link", request).ok();
            })?;
            Ok(())
        })
        // ... rest of setup
}
```

**If `tauri-plugin-deep-link` doesn't exist** (it may not be an official plugin yet), use Tauri's built-in single-instance plugin:

```rust
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            // When a second instance is launched with args, emit to the first instance
            if let Some(url) = args.get(1) {
                if url.starts_with("ronmod://") {
                    app.emit("deep-link", url).ok();
                }
            }
            // Bring window to front
            if let Some(window) = app.get_webview_window("main") {
                window.set_focus().ok();
            }
        }))
        // ... rest of setup
}
```

On Linux, the `.desktop` file launcher will invoke the app with the URL as the first argument. On Windows, the protocol handler does the same.

#### Frontend Handler

```typescript
// src/lib/deeplink.ts
import { listen } from "@tauri-apps/api/event";
import { confirm } from "@tauri-apps/plugin-dialog";
import { getModpackMetadata } from "./api/commands";
import { installModpack } from "./stores/modpack.svelte";

export function initDeepLinkHandler() {
  listen<string>("deep-link", async (event) => {
    const url = event.payload;

    if (url.startsWith("ronmod://install?url=")) {
      const modpackUrl = decodeURIComponent(
        url.replace("ronmod://install?url=", ""),
      );
      await handleInstallModpack(modpackUrl);
    } else if (url.startsWith("ronmod://import?code=")) {
      const code = url.replace("ronmod://import?code=", "");
      await handleImportCode(code);
    }
  });
}

async function handleInstallModpack(url: string) {
  try {
    // Fetch modpack metadata
    const metadata = await getModpackMetadata(url);

    // Show confirmation dialog
    const confirmed = await confirm(
      `Install modpack "${metadata.name}" (v${metadata.version}) from ${url}?\n\n` +
        `${metadata.description}\n\n` +
        `${metadata.subscriptions.length} mods, ${Object.keys(metadata.collections).length} collections`,
      { title: "Install Modpack", kind: "info" },
    );

    if (confirmed) {
      await installModpack(url);
      // Navigate to mods page or show success toast
    }
  } catch (error) {
    // Show error dialog
    await confirm(`Failed to load modpack: ${error}`, {
      title: "Error",
      kind: "error",
    });
  }
}
```

Call `initDeepLinkHandler()` in `src/main.ts` on app startup.

#### New Command: Get Modpack Metadata (without applying)

```rust
// commands/modpack.rs
#[tauri::command]
pub async fn get_modpack_metadata(url: String) -> Result<ModPack, String> {
    let modpack_url = format!("{}/ronmod.pack", url.trim_end_matches('/'));
    let response = reqwest::get(&modpack_url)
        .await
        .map_err(|e| format!("Failed to fetch modpack: {}", e))?;

    let modpack: ModPack = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse modpack: {}", e))?;

    Ok(modpack)
}
```

This allows the frontend to preview modpack details before committing to installation.

### 13.4 Testing Deep Links

**Development:**

```bash
# Linux
xdg-open "ronmod://install?url=https://mods.example.com/savagepack"

# Windows (PowerShell)
Start-Process "ronmod://install?url=https://mods.example.com/savagepack"
```

**Test cases:**

- App not running: link launches app and shows prompt
- App already running: link brings app to foreground and shows prompt
- Invalid URL: shows error dialog
- Network error: shows retry option
- User cancels: no state changes

### 13.5 User-Facing Share Links

When a user exports or shares a modpack, provide a copyable deep link:

**Share Modal UI:**

```
┌──────────────────────────────────────┐
│ Share Modpack                        │
├──────────────────────────────────────┤
│ Share via Link (one-click install):  │
│                                       │
│ ronmod://install?url=https://...     │
│ [Copy Link]                           │
│                                       │
│ Or share via code:                    │
│ Code: ABCD-1234                       │
│ ronmod://import?code=ABCD-1234        │
│ [Copy Code Link]                      │
└──────────────────────────────────────┘
```

Share links can be posted in:

- Discord servers
- Forum posts
- README files
- Social media

### 13.6 Security Considerations

1. **URL validation:** Only allow `https://` URLs (no `http://`, no `file://`)
2. **Confirmation required:** Never auto-install without user consent
3. **Modpack preview:** Always fetch and display metadata before installation
4. **Malicious URLs:** Validate that the fetched content is a valid `ronmod.pack` JSON before proceeding
5. **Rate limiting:** Prevent abuse by limiting deep link handling to once per 5 seconds

```rust
// commands/modpack.rs
pub async fn get_modpack_metadata(url: String) -> Result<ModPack, String> {
    // Security: Only allow HTTPS URLs
    if !url.starts_with("https://") {
        return Err("Only HTTPS URLs are allowed".to_string());
    }

    // Continue with fetch...
}
```

### 13.7 Implementation Phase

Add deep linking in **Phase 5** (after modpack builder and file sharing are complete):

**Phase 5 Addition:**

- 39b. Register `ronmod://` URL scheme in `tauri.conf.json`
- 39c. Implement deep link handler (single-instance plugin + event emission)
- 39d. Implement `get_modpack_metadata` command
- 39e. Build `DeepLinkDialog.svelte` — modpack preview + confirm/cancel
- 39f. Wire deep link handler in `main.ts`
- 39g. Add "Copy Install Link" button to Share Modal
- 39h. Test deep linking on Linux and Windows

This makes the share feature much more powerful — users can simply post a `ronmod://` link and recipients can install with one click.

---

## 14. Mod Profiles & Game Launcher

### 14.1 Overview

Inspired by Gale's profile system, implement **mod profiles** that allow users to maintain multiple mod configurations and switch between them seamlessly. Instead of permanently installing mods into the game directory, mods are staged in the mod manager's storage and **symlinked or copied on-demand** when launching the game.

**Key benefits:**

- **Multiple loadouts** — Vanilla, Tactical Realism, Content Expansion, Testing, etc.
- **Zero conflicts** — Only one profile's mods are active at a time
- **Quick switching** — Change profiles without re-downloading or moving files
- **Safe experimentation** — Test new mod combinations without breaking your main setup
- **Per-profile settings** — Each profile can have different collections and configurations

### 14.2 Profile System Architecture

**Profile definition:**

```rust
// models/profile.rs
#[derive(Serialize, Deserialize, Clone)]
pub struct Profile {
    pub id: String,              // UUID
    pub name: String,            // "Tactical Realism", "Vanilla", etc.
    pub description: Option<String>,
    pub icon: Option<String>,    // Emoji or icon name
    pub mods: Vec<String>,       // List of mod identifiers (filenames or mod.io IDs)
    pub collections: HashMap<String, bool>, // Enabled collections
    pub created_at: DateTime<Utc>,
    pub last_used: Option<DateTime<Utc>>,
    pub is_vanilla: bool,        // Special flag for "no mods" profile
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ProfileConfig {
    pub active_profile_id: Option<String>,
    pub profiles: Vec<Profile>,
}
```

**Storage locations:**

- **Mod staging area:** `{app_data}/ronmodmanager/mods/` — all downloaded mods stored here
- **Profile config:** `{app_data}/ronmodmanager/profiles.json`
- **Game mods directory:** `{game_path}/ReadyOrNot/Content/Paks/~mods/` — symlinks/copies placed here

### 14.3 Profile Workflow

#### Creating a Profile

1. User clicks "New Profile" in the Profiles page
2. Enters name, description, optional icon (emoji picker)
3. Selects which mods to include (checkbox list of all available mods)
4. Selects which collections to enable
5. Profile is saved to `profiles.json`

#### Switching Profiles

**Option A: Manual switch (no game launch)**

- User selects profile from dropdown
- App clears `~mods/` directory
- App symlinks selected mods from staging area into `~mods/`
- User manually launches game via Steam
- **Pros:** User retains control of when to launch
- **Cons:** Extra step, mods persist until next switch

**Option B: Launch with profile (recommended)**

- User clicks "Launch with [Profile Name]" button
- App clears `~mods/` directory
- App symlinks/copies mods from profile into `~mods/`
- App launches game (see §14.4)
- Optionally: App monitors game process and cleans up `~mods/` when game exits
- **Pros:** Clean workflow, guaranteed correct mods, optional auto-cleanup
- **Cons:** Must launch through mod manager

**Recommendation:** Implement both options, default to Option B.

#### Vanilla Profile

Always provide a built-in "Vanilla" profile that:

- Has `is_vanilla: true` flag
- Contains zero mods
- When activated/launched, clears `~mods/` completely
- Cannot be deleted or edited

### 14.4 Game Launcher Integration

Launch Ready or Not directly from the mod manager with the selected profile's mods.

**Launch methods (in order of preference):**

#### Method 1: Steam URL (cross-platform, recommended)

```rust
// commands/launcher.rs
#[tauri::command]
pub async fn launch_game_with_profile(
    profile_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // 1. Apply profile (symlink mods)
    apply_profile(&profile_id, &state).await?;

    // 2. Launch via Steam
    let steam_url = "steam://rungameid/1144200";

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(&["/C", "start", steam_url])
            .spawn()
            .map_err(|e| format!("Failed to launch: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(steam_url)
            .spawn()
            .map_err(|e| format!("Failed to launch: {}", e))?;
    }

    Ok(())
}
```

**Pros:**

- Works on both Linux and Windows
- Respects Steam's launch options set by user
- Handles Steam Overlay, achievements, etc.
- No need to find game executable

**Cons:**

- Can't detect when game closes (for auto-cleanup)
- Slight delay from Steam launching

#### Method 2: Direct executable launch (optional fallback)

```rust
// Only if Steam URL fails or user prefers direct launch
let game_exe = if cfg!(windows) {
    game_path.join("ReadyOrNot/Binaries/Win64/ReadyOrNot-Win64-Shipping.exe")
} else {
    // Linux/Proton path — more complex, requires finding compatdata
    game_path.join("ReadyOrNot.exe") // Launched via Proton
};

let mut child = std::process::Command::new(&game_exe)
    .spawn()
    .map_err(|e| format!("Failed to launch: {}", e))?;

// Wait for game to exit (optional)
tokio::spawn(async move {
    let _ = child.wait();
    // Game exited, optionally clean up ~mods/
});
```

**Pros:**

- Can monitor game process for cleanup
- Slightly faster than Steam URL

**Cons:**

- Bypasses Steam (no overlay, achievements may not work)
- Complex on Linux/Proton (need to invoke via Proton/Wine)
- Launch options from Steam won't apply

**Recommendation:** Use Steam URL by default, offer direct launch as advanced option.

### 14.5 Mod Loading Strategy: Symlinks vs Copy

#### Symlinks (Recommended)

**Pros:**

- Instant (no file copying)
- Zero disk space duplication
- Changes to staged mods immediately reflect in game

**Cons:**

- Requires elevated permissions on Windows (or Developer Mode enabled)
- Some game engines don't follow symlinks (Ready or Not uses Unreal Engine 5, which _does_ support symlinks)

**Implementation:**

```rust
#[cfg(target_os = "windows")]
{
    std::os::windows::fs::symlink_file(source, dest)?;
}

#[cfg(target_os = "linux")]
{
    std::os::unix::fs::symlink(source, dest)?;
}
```

**Fallback on Windows:** If symlink creation fails (access denied), fall back to hard links or copying.

#### Copy (Fallback)

**Pros:**

- No permission issues
- Works everywhere

**Cons:**

- Slower (especially for large mod packs)
- Wasted disk space
- Stale copies if staging area is updated

**Implementation:**

```rust
std::fs::copy(source, dest)?;
```

**Strategy:** Try symlink first, fall back to copy if it fails. Show warning in UI if copying is being used.

### 14.6 Desktop Shortcuts & Steam Launch Options

#### Desktop Shortcut (Recommended)

Create shortcuts that launch the game via the mod manager with a specific profile:

```
ronmodmanager://launch?profile=tactical-realism
```

**Windows shortcut (.lnk):**

- Target: `"C:\Program Files\RoN Mod Manager\ronmodmanager.exe" --profile tactical-realism`
- Icon: Game icon or profile icon

**Linux .desktop file:**

```ini
[Desktop Entry]
Name=Ready or Not (Tactical Realism)
Exec=ronmodmanager --profile tactical-realism
Icon=ronmodmanager
Type=Application
Categories=Game;
```

**UI Flow:**

1. User right-clicks profile
2. Selects "Create Desktop Shortcut"
3. App generates platform-specific shortcut
4. Shortcut placed on Desktop

#### Steam Launch Option (Alternative)

**Not recommended** because Steam launch options run _before_ the game, and we need to run the mod manager _instead of_ launching directly. However, for advanced users:

1. Add RoN Mod Manager as a "non-Steam game" to Steam library
2. Rename it to "Ready or Not (Modded)"
3. Set launch options: `--profile tactical-realism --launch`
4. User launches "Ready or Not (Modded)" from Steam
5. Mod manager applies profile and launches real game

This gives users Steam Overlay and Big Picture support while using profiles.

### 14.7 UI Components

#### Profiles Page

```
┌─────────────────────────────────────────────┐
│ Profiles                           [+ New]  │
├─────────────────────────────────────────────┤
│                                             │
│ ┌─────────────────────────────────────┐    │
│ │ 🎯 Tactical Realism    [Launch]     │    │
│ │ 15 mods • 2 collections             │    │
│ │ Last used: 2 hours ago              │    │
│ │ [Edit] [Duplicate] [Delete] [⋮]     │    │
│ └─────────────────────────────────────┘    │
│                                             │
│ ┌─────────────────────────────────────┐    │
│ │ 📦 Content Expansion   [Launch]     │    │
│ │ 8 mods • 0 collections              │    │
│ │ Last used: Yesterday                │    │
│ │ [Edit] [Duplicate] [Delete] [⋮]     │    │
│ └─────────────────────────────────────┘    │
│                                             │
│ ┌─────────────────────────────────────┐    │
│ │ ⭐ Vanilla (Default)   [Launch]      │    │
│ │ No mods                              │    │
│ │ [Launch] [⋮]                         │    │
│ └─────────────────────────────────────┘    │
│                                             │
└─────────────────────────────────────────────┘
```

**Profile Actions (⋮ menu):**

- Launch with profile
- Set as default
- Create desktop shortcut
- Share profile (export as modpack)
- Duplicate
- Rename
- Delete

#### Profile Editor Modal

```
┌────────────────────────────────────────┐
│ Edit Profile: Tactical Realism         │
├────────────────────────────────────────┤
│ Name: [Tactical Realism            ]   │
│ Icon: [🎯] [Choose emoji]              │
│ Description:                           │
│ [Realistic gameplay mods for immersive │
│  tactical operations                  ]│
│                                        │
│ Mods (12 available):                   │
│ ☑ Enhanced AI v2.3                     │
│ ☑ Realistic Weapons                    │
│ ☑ Tactical HUD                         │
│ ☐ Fairfax Residence Remake             │
│ ☐ John Wick Suit                       │
│ ... [Show all]                         │
│                                        │
│ Collections:                           │
│ ☑ Beat Cop                             │
│ ☐ John Wick                            │
│                                        │
│        [Cancel]      [Save Profile]    │
└────────────────────────────────────────┘
```

#### Quick Launch (Dashboard Widget)

```
┌──────────────────────────────────┐
│ Quick Launch                     │
├──────────────────────────────────┤
│ Profile: [Tactical Realism ▼]    │
│ [🚀 Launch Game]                 │
│                                  │
│ Or launch:                       │
│ • Vanilla                        │
│ • Content Expansion              │
│ • Testing                        │
└──────────────────────────────────┘
```

### 14.8 Data Model Extensions

Update `AppConfig`:

```rust
#[derive(Serialize, Deserialize, Clone)]
pub struct AppConfig {
    // ... existing fields
    pub active_profile_id: Option<String>,
    pub profiles: Vec<Profile>,
    pub auto_cleanup_on_exit: bool, // Clean ~mods/ when game exits
    pub use_symlinks: bool,          // vs copying
    pub last_launch: Option<DateTime<Utc>>,
}
```

New models:

```rust
// models/profile.rs
#[derive(Serialize, Deserialize, Clone)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub mod_files: Vec<String>, // Filenames in staging area
    pub collections: HashMap<String, bool>,
    pub created_at: DateTime<Utc>,
    pub last_used: Option<DateTime<Utc>>,
    pub is_vanilla: bool,
    pub is_default: bool, // Auto-apply on app launch
}
```

### 14.9 Commands

```rust
// commands/profile.rs

#[tauri::command]
pub async fn get_profiles(state: State<'_, AppState>) -> Result<Vec<Profile>, String>;

#[tauri::command]
pub async fn create_profile(
    name: String,
    description: Option<String>,
    icon: Option<String>,
    mod_files: Vec<String>,
    collections: HashMap<String, bool>,
    state: State<'_, AppState>,
) -> Result<Profile, String>;

#[tauri::command]
pub async fn update_profile(
    profile_id: String,
    name: String,
    description: Option<String>,
    icon: Option<String>,
    mod_files: Vec<String>,
    collections: HashMap<String, bool>,
    state: State<'_, AppState>,
) -> Result<Profile, String>;

#[tauri::command]
pub async fn delete_profile(
    profile_id: String,
    state: State<'_, AppState>,
) -> Result<(), String>;

#[tauri::command]
pub async fn apply_profile(
    profile_id: String,
    state: State<'_, AppState>,
) -> Result<(), String>; // Symlink/copy mods, don't launch

#[tauri::command]
pub async fn launch_game_with_profile(
    profile_id: String,
    state: State<'_, AppState>,
) -> Result<(), String>; // Apply profile + launch game

#[tauri::command]
pub async fn create_desktop_shortcut(
    profile_id: String,
    state: State<'_, AppState>,
) -> Result<(), String>;

#[tauri::command]
pub async fn export_profile_as_modpack(
    profile_id: String,
    state: State<'_, AppState>,
) -> Result<ModPack, String>;
```

### 14.10 Implementation Phase

Add profiles in **Phase 4.5** (after frontend UI is built, before modpack builder):

**Phase 4.5: Profiles & Game Launcher**

- 33a. Implement `models/profile.rs`
- 33b. Implement `services/profile.rs` — profile management logic
- 33c. Implement `services/launcher.rs` — game launch + mod symlinking
- 33d. Implement `commands/profile.rs` — all profile commands
- 33e. Build `Profiles.svelte` page — profile list with cards
- 33f. Build `ProfileEditor.svelte` modal — create/edit profiles
- 33g. Build `QuickLaunch.svelte` dashboard widget
- 33h. Add profile selector to sidebar + launch button
- 33i. Implement desktop shortcut creation (platform-specific)
- 33j. Add `ronmod://launch?profile=X` deep link handler
- 33k. Test symlink creation on Linux and Windows (check permissions)
- 33l. Test game launch via Steam URL
- 33m. Optional: Implement game process monitoring + auto-cleanup

### 14.11 Edge Cases & Considerations

1. **Windows symlink permissions:**
   - Require Developer Mode enabled or admin rights
   - Fall back to copying if symlinks fail
   - Show one-time notification explaining Developer Mode

2. **Profile corruption:**
   - If a mod in a profile no longer exists, show warning
   - Allow user to fix profile or remove missing mods

3. **Game already running:**
   - Detect if Ready or Not is already running before switching profiles
   - Warn user: "Game is running. Please close it before switching profiles."

4. **Modpack → Profile conversion:**
   - When importing a modpack, offer to create a profile from it
   - "Import as profile: [Profile Name]"

5. **Steam Deck compatibility:**
   - Steam URL launch works perfectly on Steam Deck
   - Profiles can be switched from Gaming Mode
   - Consider adding gamepad navigation support

6. **Cleanup behavior:**
   - Option 1: Leave mods in `~mods/` until next profile switch (simpler)
   - Option 2: Monitor game process, clean up on exit (complex, optional)
   - User preference in settings

### 14.12 UI/UX Enhancements

- **Profile badges:** "Default", "Last Used", "Vanilla"
- **Mod count indicators:** "15 mods • 2 conflicts"
- **Launch button states:** Disabled if game path not set, pulsing animation on launch
- **Profile sorting:** Last used, alphabetical, custom order (drag-to-reorder)
- **Search/filter:** Filter profiles by name or included mods
- **Profile templates:** "Create from current mods", "Create from modpack"
- **Conflict detection:** Warn if two mods in profile are known to conflict

---

## 15. Migration Notes for Existing Users

- This is a **clean-break** new application — no backward compatibility with `rmd.pack` or the old CLI tool's config format
- The new app stores config in OS-standard locations (`~/.config/ronmodmanager/` or `%APPDATA%/ronmodmanager/`), not alongside the executable
- Modpack format is `ronmod.pack` (new name, new app)
- Existing modpack server folder structure is unchanged — no server-side changes required for basic operation
- The `_overrides/`, `_collections/`, `_manual/` folder convention is preserved exactly
- Users will need to re-authenticate with mod.io on first launch (via Steam OAuth in the webview)
