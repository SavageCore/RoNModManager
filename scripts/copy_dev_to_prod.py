import json
import shutil
from pathlib import Path

# --- Configuration Paths ---
# Production targets
prod_base = Path("/home/savagecore/.local/share/ronmodmanager")
prod_config_file = Path("/home/savagecore/.config/ronmodmanager/config.json")
prod_flatpak_data = Path("/home/savagecore/.var/app/uk.savagecore.ronmodmanager/data/ronmodmanager")
prod_flatpak_config_file = Path("/home/savagecore/.var/app/uk.savagecore.ronmodmanager/config/ronmodmanager/config.json")

# Dev sources
dev_base = Path("/home/savagecore/.local/share/ronmodmanager-dev")
dev_config_file = Path("/home/savagecore/.config/ronmodmanager-dev/config.json")

# Replacement tokens
old_str = str(dev_base)
new_str = str(prod_base)


def sync_directory(src: Path, dest: Path) -> None:
    """Wipes the destination and clones the source directory."""
    if dest.exists():
        shutil.rmtree(dest)
    shutil.copytree(src, dest)
    print(f"Synced Directory: {src} -> {dest}")


def fix_manifest_references(directory: Path) -> None:
    """Finds and replaces dev paths within a directory's manifests."""
    manifest_dir = directory / "staged" / ".manifests"
    if not manifest_dir.exists():
        return

    for file_path in manifest_dir.iterdir():
        if file_path.is_file():
            content = file_path.read_text(encoding="utf-8")
            if old_str in content:
                file_path.write_text(content.replace(old_str, new_str), encoding="utf-8")
                print(f"  Updated manifest: {file_path.name} in {directory.name}")


def sync_and_patch_config(src_file: Path, dest_file: Path) -> None:
    """Reads source JSON config, swaps dev paths for prod paths, and saves to target."""
    if not src_file.is_file():
        print(f"Warning: Source config file not found at {src_file}")
        return

    with src_file.open("r", encoding="utf-8") as f:
        config_data = json.load(f)

    # Convert to string to replace paths globally across the JSON structure
    config_str = json.dumps(config_data)
    if old_str in config_str:
        config_data = json.loads(config_str.replace(old_str, new_str))

    # Ensure parent directory exists before writing
    dest_file.parent.mkdir(parents=True, exist_ok=True)
    
    with dest_file.open("w", encoding="utf-8") as f:
        json.dump(config_data, f, indent=4)
    print(f"Synced and patched config -> {dest_file}")


# --- Execution Flow ---

# 1. Sync data directories and patch internal manifest paths
sync_directory(dev_base, prod_base)
fix_manifest_references(prod_base)

sync_directory(dev_base, prod_flatpak_data)
fix_manifest_references(prod_flatpak_data)

# 2. Sync and patch configuration files
sync_and_patch_config(dev_config_file, prod_config_file)          # Native Config
sync_and_patch_config(dev_config_file, prod_flatpak_config_file)  # Flatpak Config