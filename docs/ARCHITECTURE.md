# RUSTIUM Architecture

## Phase 1 — Engine & Desktop Integration Selection

### Goal
Build a genuinely functional desktop web browser with **real website rendering**, not a mock UI. Rust must own core application logic, with a modular engine abstraction so rendering backends can evolve.

### Options Evaluated

| Option | Rendering | Pros | Cons | Verdict |
|---|---|---|---|---|
| **Tauri 2 + system WebView (WRY/WebKitGTK on Linux)** | Real engine (WebKitGTK 4.1 on Linux, WKWebView on macOS, WebView2/Chromium on Windows) | Fast to ship, real rendering, Rust-native shell, small footprint, cross-platform packaging via Tauri, leverages OS-updated engines | Not a custom/bundled Chromium build; extension model not Chrome-Web-Store-compatible; per-engine capability differences; multi-webview tab management requires careful window/webview composition | **Selected for v1** |
| **Chromium Embedded Framework (CEF)** | Bundled Chromium | Closest to Chrome behavior, strong extension/DevTools potential | Large binaries/build complexity, heavy integration effort, licensing/distribution overhead, longer path to a first launchable browser | Future backend target |
| **Servo embedding** | Servo engine | Rust-native engine, promising architecture | Embedding ecosystem immature for full browser product features, site compatibility gaps | Future/experimental backend target |
| **Build engine from scratch** | Custom | Full control | Unrealistic for v1; would prevent real rendering for years | Rejected |

### Selected Architecture (v1)

- **Shell/Desktop:** Tauri 2 (Rust backend + WebView host)
- **UI chrome:** React + TypeScript + Tailwind CSS rendered in the main application webview
- **Content rendering:** Real web content rendered by the platform/system webview engine through managed content webviews (WebKitGTK 4.1 detected on this Linux environment). This is **not** presented as a custom Chromium engine.
- **Core logic (Rust):** tabs/navigation state, bookmarks, history, downloads, settings, profiles/privacy/session policy, permissions model, persistence, command/event IPC.
- **Persistence:** SQLite (via `rusqlite`/`sqlx`-style access as implemented) for history, bookmarks, downloads, settings, sessions metadata.
- **Async/runtime:** Tokio for background tasks (downloads, DB writes, maintenance) with careful UI-thread boundaries.
- **Engine abstraction:** `engine` module defines traits/interfaces for navigation, tab lifecycle, permissions, and capability reporting so CEF/Servo adapters can be added later without rewriting product logic.

### Key Limitations (honest)

- v1 cannot guarantee Chrome-identical rendering or full Chromium extension compatibility because it uses the system webview engine per platform.
- Some features depend on engine support (e.g., true process-per-tab isolation controls, extension APIs, granular permission prompts). Unsupported capabilities must be reported in UI and handled via best realistic alternatives.
- Advanced protections (site isolation internals, full Safe Browsing parity) may be partial in v1; security defaults remain strict (TLS verification never disabled).

### Modular Boundaries

- `browser/` — app orchestration and command handlers
- `engine/` — engine abstraction + Tauri/WebView adapter
- `tabs/` — tab model, ordering, groups, session restore state
- `navigation/` — omnibox resolution, URL/search handling
- `bookmarks/`, `history/`, `downloads/` — domain logic
- `privacy/`, `security/`, `profiles/` — policies and permission state
- `database/` — schema, migrations, repositories
- `settings/` — typed settings and defaults
- `extensions/` — extension management architecture/capability layer

### Incremental Delivery Strategy

1. Scaffold Rustium app shell and DB.
2. Implement tab/navigation model + real content rendering for at least core browsing flows.
3. Add persistence features (history/bookmarks/downloads/settings).
4. Add privacy/private mode and permission plumbing.
5. Complete UI/UX, tests, packaging, docs.

This document will be updated as implementation validates engine capabilities.
