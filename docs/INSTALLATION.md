# Installation & Build Guide

## Prerequisites

### Linux (tested target)
- Rust stable (`rustup`)
- Node.js 18+ and npm
- System libraries for Tauri/Wry webview:
  - WebKitGTK 4.1 (`webkit2gtk-4.1`)
  - GTK3, libsoup, javascriptcoregtk, etc. (per Tauri prerequisites)

See: https://tauri.app/start/prerequisites/

### Windows
- Microsoft Visual Studio C++ Build Tools
- WebView2 Runtime (Evergreen)

### macOS
- Xcode Command Line Tools

## Development

```bash
npm install
npm run tauri dev
```

This starts the Vite dev server and launches the Rustium desktop app.

## Production Build

```bash
npm install
npm run build          # frontend bundle
npm run tauri build    # native app + bundles
```

Build artifacts:
- Linux: `src-tauri/target/release/bundle/` (e.g. `.deb`, `.AppImage` depending on toolchain/bundlers installed)
- Windows: installer/exe under `bundle/msi` or `bundle/nsis` when configured tools are present
- macOS: `.app`/`.dmg` under `bundle/`

> Note: Cross-compiling installers requires platform-native tooling. Generate Windows/macOS installers on their respective platforms or CI runners.

## Database & User Data

RUSTIUM stores persistent data (settings, bookmarks, history, downloads metadata) in a SQLite database under the platform app data directory (via `directories` crate), e.g. `~/.local/share/com.rustium.RUSTIUM/rustium.db` on Linux.

Private browsing tabs do not persist history entries to the database (v1 behavior). Session-scoped storage isolation beyond history persistence depends on engine capabilities and is an area for further hardening.
