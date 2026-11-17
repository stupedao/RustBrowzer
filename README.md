# RustBrowzer

A lightweight, privacy-first desktop web browser built with **Rust + Tauri 2 + React 19**.
Rendering uses the platform system webview (WebKitGTK on Linux) — no bundled engine,
no GPU required (CPU rendering is forced via environment flags).

## Architecture (v0.2 — rewritten)

- **One live webview.** Only the active tab is materialized as a content webview;
  all other tabs are metadata-only and reload on activation. RAM stays flat
  (~tens of MB per active page instead of per tab).
- **Event-driven.** Real WebKit events (`on_page_load`, `on_document_title_changed`,
  `on_download`, `on_navigation`) drive the UI through typed Tauri events. No polling,
  no timers, no fake loading states.
- **Security.** Content webviews get zero IPC (capability is scoped to the `main`
  webview; every command additionally verifies the caller label). Only `http(s)`
  URLs can ever be rendered (`file:`, `javascript:` etc. are blocked at the
  navigation hook *and* the URL sanitizer). New windows/popups are denied.
  Private tabs use ephemeral (incognito) web sessions. Strict CSP on the UI.
- **Real features.** Working back/forward (Rust-side history stacks), frecency
  omnibox suggestions (history + bookmarks + open tabs), real downloads with
  sanitized non-overwriting filenames, session restore, tab pin/mute/move,
  Ctrl+T/W/L/R, Ctrl+Shift+T, Ctrl+Tab, GTK accelerators that work even while
  a page has focus.

## Layout

```
src-tauri/src/
  renderer/     content webview lifecycle (LRU, one live webview)
  navigation/   URL resolution/sanitization, search engines + keywords
  state.rs      tabs, history stacks, settings
  database/     SQLite (WAL, prepared statements, history cap/dedupe)
  downloads/    WebKit download pipeline (sanitized paths)
  commands.rs   IPC surface (UI-origin guarded)
  events.rs     typed backend→UI events
  linux_webview.rs  GtkFixed container + keyboard accelerators
src/
  browser.ts    event-driven store (no polling, viewport measurement)
  App.tsx       browser chrome (tabs, omnibox, panels, context menu)
  Home.tsx      start page (no branding, minute-aligned clock)
```

## Development

Requires Rust (stable), Node.js, and on Linux: `webkit2gtk-4.1` + `gtk3` dev packages.

```bash
npm install
npm run tauri dev
```

## Build

```bash
npm run tauri build
```

Rust unit tests: `cd src-tauri && cargo test` (navigation resolution, scheme blocking).

## Honest limitations

- Mute is a UI flag (WebKitGTK exposes no per-webview mute through wry).
- Stop-load is best-effort (no stop API in wry); loading state self-corrects.
- Switching away from a tab discards it (by design, for RAM); switching back reloads.
