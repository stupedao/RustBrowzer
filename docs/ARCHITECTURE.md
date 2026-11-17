# Architecture

## Rendering model

The browser chrome is the window's own webview (label `main`). Real pages are
separate *content webviews* created with `Window::add_child` (unstable API) and
positioned over the content viewport.

`WebviewManager` (src-tauri/src/renderer/mod.rs) owns the live set:

- `show(tab, viewport)` — evicts all other live webviews (LRU), then reuses or
  creates the tab's webview at the viewport bounds.
- `max_live = 1` — the memory budget. Raising it later is a one-line change;
  eviction is already implemented and stamped.
- Labels are derived from tab ids (`c<uuid-no-dashes>`), so a stale webview can
  never be mistaken for a live one.

On Linux, `add_child` packs child webviews into the window's `GtkBox` (stacking
them). `linux_webview.rs` installs a `GtkFixed` container and moves every
webview into it so bounds are honored; it also installs window accelerators.

## Lifecycle

All tab state lives in Rust (`state.rs`). The UI is a projection: it receives
`tabs-changed` events and issues commands. Page events flow through wry hooks
into typed events (`load-started`, `load-finished`, `title-changed`,
`url-changed`, `tab-discarded`, `downloads-changed`), all emitted *to the UI
webview only*.

Back/forward: because inactive tabs are discarded, Rust keeps per-tab URL
stacks and re-materializes the webview on navigation.

## Security

1. Capability file grants `core:default` to webview `main` only; content
   webviews are remote origins → no IPC at all (verified against Tauri
   2.12.5 ACL resolution: `webviews || windows` glob matching).
2. Every command re-checks the caller webview label (`assert_ui_origin`).
3. `sanitize_url` allows only `http/https` with a host — everywhere, including
   bookmarks and the homepage setting.
4. `on_navigation` blocks non-web schemes mid-load (file:, custom schemes…).
5. `on_new_window` denies popups/target=_blank.
6. Private tabs: `incognito(true)` → ephemeral WebKit session, no DB writes.
7. Downloads: filename sanitized, redirected into the downloads dir,
   never overwrites existing files.
8. No initialization scripts are injected into web content; the old fake
   "CSP meta tag" injection (which did nothing) was removed — the UI's CSP
   comes from tauri.conf.json + index.html.

## Performance

- One live webview (bounded RAM/CPU).
- CPU rendering forced: WEBKIT_DISABLE_COMPOSITING_MODE=1,
  WEBKIT_DISABLE_DMABUF_RENDERER=1, LIBGL_ALWAYS_SOFTWARE=1 (set in main.rs
  before toolkit init; user-set values are respected).
- No polling anywhere; ResizeObserver + rAF-debounced bounds sync.
- SQLite: WAL, synchronous=NORMAL, busy_timeout, prepared statements,
  history deduped by URL with visit counts, capped at 5000 rows.
- UI: memoized tab chips, minute-aligned clock, no infinite animations,
  no large blur radii, contained re-renders.
- Release profile: opt-level "s", LTO, single codegen unit, strip.
