#!/usr/bin/env bash
set -euo pipefail

CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"

CONFIG_DIR="$CONFIG_HOME/ronmodmanager"
DATA_DIR="$DATA_HOME/ronmodmanager"
MANIFESTS_DIR="$DATA_DIR/staged/.manifests"
PROFILES_DIR="$DATA_DIR/profiles"
DEFAULT_PROFILE_PATH="$PROFILES_DIR/Default.json"
CONFIG_PATH="$CONFIG_DIR/config.json"

if ! command -v node >/dev/null 2>&1; then
  echo "Error: node is required to run this migration script." >&2
  exit 1
fi

if [[ ! -d "$MANIFESTS_DIR" ]]; then
  echo "No manifests directory found at: $MANIFESTS_DIR"
  echo "Nothing to migrate."
  exit 0
fi

mkdir -p "$PROFILES_DIR" "$CONFIG_DIR"

MANIFEST_COUNT=$(find "$MANIFESTS_DIR" -maxdepth 1 -type f -name '*.json' | wc -l | tr -d ' ')
if [[ "$MANIFEST_COUNT" == "0" ]]; then
  echo "No manifest files found in: $MANIFESTS_DIR"
  echo "Nothing to migrate."
  exit 0
fi

if [[ -f "$DEFAULT_PROFILE_PATH" ]]; then
  cp "$DEFAULT_PROFILE_PATH" "$DEFAULT_PROFILE_PATH.bak.$(date +%Y%m%d%H%M%S)"
fi

if [[ -f "$CONFIG_PATH" ]]; then
  cp "$CONFIG_PATH" "$CONFIG_PATH.bak.$(date +%Y%m%d%H%M%S)"
fi

node - "$MANIFESTS_DIR" "$DEFAULT_PROFILE_PATH" "$CONFIG_PATH" <<'NODE'
const fs = require('fs');
const path = require('path');

const manifestsDir = process.argv[2];
const defaultProfilePath = process.argv[3];
const configPath = process.argv[4];

function readJson(filePath, fallback = null) {
  try {
    return JSON.parse(fs.readFileSync(filePath, 'utf8'));
  } catch {
    return fallback;
  }
}

function writeJsonAtomic(filePath, value) {
  const tmp = `${filePath}.tmp`;
  fs.writeFileSync(tmp, JSON.stringify(value, null, 2));
  fs.renameSync(tmp, filePath);
}

const names = new Set();
for (const entry of fs.readdirSync(manifestsDir)) {
  if (!entry.endsWith('.json')) continue;
  const fullPath = path.join(manifestsDir, entry);
  const data = readJson(fullPath, null);
  if (data && typeof data.source_archive === 'string' && data.source_archive.trim()) {
    names.add(data.source_archive.trim());
  }
}

const enabledGroups = Array.from(names).sort((a, b) => a.localeCompare(b));

const existingProfile = readJson(defaultProfilePath, {});
const nowIso = new Date().toISOString();

const defaultProfile = {
  name: 'Default',
  description:
    typeof existingProfile.description === 'string'
      ? existingProfile.description
      : `Migrated from installed manifests on ${nowIso}`,
  enabled_collections: enabledGroups,
  created_at:
    typeof existingProfile.created_at === 'string' && existingProfile.created_at
      ? existingProfile.created_at
      : nowIso,
};

const config = readJson(configPath, {});
const mergedConfig = {
  ...config,
  active_profile: 'Default',
  enabled_collections: enabledGroups,
};

writeJsonAtomic(defaultProfilePath, defaultProfile);
writeJsonAtomic(configPath, mergedConfig);

console.log(`Migrated ${enabledGroups.length} mod group(s) into Default profile.`);
console.log(`Profile: ${defaultProfilePath}`);
console.log(`Config:  ${configPath}`);
NODE

echo "Done. Restart the app (or switch away/back to Mods page) to reload profile state."