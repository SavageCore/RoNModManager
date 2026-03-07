# Linux Window Position Persistence

## Overview

Window size persistence works on all Linux systems. However, **window position persistence only works when running under X11 or XWayland** due to Wayland's security model.

## Wayland Limitations

Native Wayland applications cannot control window positioning - this is a deliberate design decision by Wayland. The compositor (window manager) has full control over window placement.

## Solution: XWayland Mode

To enable full window state persistence (size AND position) on Wayland systems, the application runs in XWayland compatibility mode by default.

### Desktop File Configuration

For applications to run in XWayland mode by default, the `.desktop` file should set `GDK_BACKEND=x11`:

```desktop
Exec=env GDK_BACKEND=x11 ronmodmanager %U
```

This environment variable tells GTK to use X11 protocols instead of native Wayland, enabling window positioning APIs.

A template desktop file is provided at `src-tauri/ronmodmanager.desktop` for reference.

### Package Types

**Flatpak** (Automated):

- Uses `packaging/flatpak/uk.savagecore.ronmodmanager.desktop`
- Already configured with `GDK_BACKEND=x11`
- Works out of the box

**DEB/AppImage** (Post-install configuration):

- Tauri generates a basic desktop file automatically during build
- Users may need to manually edit `~/.local/share/applications/ronmodmanager.desktop` to add XWayland mode
- Or copy the template from `src-tauri/ronmodmanager.desktop` which includes the environment variable

To manually enable XWayland for installed DEB/AppImage:

```bash
# Edit the desktop file
nano ~/.local/share/applications/ronmodmanager.desktop

# Change the Exec line from:
Exec=ronmodmanager
# To:
Exec=env GDK_BACKEND=x11 ronmodmanager %U
```

### Development Mode

Two dev commands are available:

```bash
# Native Wayland (window position won't persist)
pnpm run dev:linux

# XWayland mode (full window state persistence)
pnpm run dev:xwayland
```

### Flatpak Permissions

If building/running as Flatpak, ensure the application has X11 socket access in Flatseal or manifest:

```yaml
finish-args:
  - --socket=x11
  - --share=ipc
```

This allows the Flatpak to communicate with the XWayland server for window management.

## Desktop Environment Compatibility

The `GDK_BACKEND=x11` approach works universally across:

- **GNOME** (Mutter compositor)
- **KDE Plasma** (KWin compositor)
- **Sway**
- **Hyprland**
- Any other Wayland compositor that supports XWayland

No desktop environment-specific code is needed - XWayland provides a consistent X11 interface regardless of the underlying compositor.

## User Impact

- **Visual**: No noticeable difference - XWayland applications look and behave like native Wayland apps
- **Performance**: Negligible overhead in typical desktop app scenarios
- **Functionality**: Enables window position persistence without compromising Wayland's security model
- **Fractional Scaling**: Works properly with modern Wayland fractional scaling

## Alternative Approach (Not Recommended)

Users could manually launch without the environment variable to run as pure Wayland:

```bash
ronmodmanager  # pure Wayland, position won't persist
```

However, this is not recommended as it degrades the user experience by losing window position memory.
