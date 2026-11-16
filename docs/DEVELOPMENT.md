# Developer Documentation

## Repository Layout

- `src/` — React + TypeScript browser UI (chrome, tabs, panels, settings views)
- `src-tauri/src/` — Rust core
  - `browser/` app state orchestration
  - `engine/` engine abstraction and capability reporting
  - `tabs/` tab models/state
  - `navigation/` URL/search resolution
  - `database/` SQLite open/migrations/repositories
  - `bookmarks/`, `history/`, `downloads/` domain modules
  - `privacy/`, `security/`, `profiles/`, `extensions/`, `settings/` policy and config modules
- `docs/` — architecture and guides

## IPC Contract (Tauri commands)

Core commands implemented in v1 foundation:
- `get_snapshot`, `get_settings`, `save_settings`
- `new_tab`, `close_tab`, `set_active_tab`, `update_tab`, `navigate_tab`
- `resolve_url`
- `list_history`, `clear_history`
- `list_bookmarks`, `add_bookmark`, `remove_bookmark`
- `open_content`, `hide_content`, `resize_content`, `close_content` (real webview content rendering management)

## Rendering Model

The main window hosts the Rustium UI. Website content is rendered in managed **real webviews** created/navigated by the Rust backend (Tauri/Wry system engine). On Linux this uses WebKitGTK 4.1. This provides genuine web rendering, but is not a custom bundled Chromium engine.

## Security Defaults

- TLS verification is never disabled.
- External navigation is passed as parsed URLs to the engine.
- Private tabs avoid writing history records (v1).
- Further sandboxing/permission prompts depend on engine and OS capabilities; see `docs/ARCHITECTURE.md`.

## Testing

```bash
cd src-tauri && cargo test
npm run build
```

Integration/UI and navigation tests should be expanded with platform automation as engine integration matures.

## Extending the Engine

Implement an adapter behind the `engine` abstraction to target CEF or Servo. Keep product logic (tabs, persistence, policies) independent of backend-specific APIs.
