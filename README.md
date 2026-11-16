# RUSTIUM

RUSTIUM is a Rust-powered desktop web browser project built with **Tauri 2 + React + TypeScript**.

> **Status:** early implementation in progress. This repository aims for a genuinely functional browser with real website rendering (via platform/system webview engine in v1) and a modular architecture that can target alternative engines (e.g. CEF/Servo) later.

## Architecture

- **Core logic:** Rust (`src-tauri/`) for tabs/navigation/state, persistence, downloads, privacy/security policies.
- **UI:** React + TypeScript + Tailwind (`src/`).
- **Rendering (v1):** Real web content via Tauri/Wry system webview (WebKitGTK 4.1 on Linux). Not marketed as a custom Chromium engine.
- **Database:** SQLite for persistent browser data.
- See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for tradeoffs and module boundaries.

## Prerequisites

- Rust (stable) + Cargo
- Node.js + npm
- Linux: WebKitGTK 4.1 and GTK development/runtime libs (for Tauri/Wry), e.g. distro packages providing `webkit2gtk-4.1` and `gtk3`
- Platform-specific Tauri prerequisites: https://tauri.app/start/prerequisites/

## Development

```bash
npm install
npm run tauri dev
```

Frontend-only dev server:

```bash
npm run dev
```

## Build

```bash
npm run tauri build
```

Artifacts are produced under `src-tauri/target/release/bundle/` (platform-dependent).

## Project Structure

See `docs/ARCHITECTURE.md` and source directories under `src/` and `src-tauri/src/`.

## Testing

Test commands and coverage will be expanded as features land (Rust unit/integration tests and frontend/UI tests).

## Notes

- TLS verification is never disabled.
- Features that depend on engine capabilities are implemented with honest fallbacks and documented limitations.
- This is not a claim of Chrome Web Store compatibility unless explicitly implemented and tested.
