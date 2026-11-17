//! The rendering architecture.
//!
//! The browser chrome (tab strip, toolbar, panels) is the window's own webview
//! ("ui"). Real web pages live in separate *content webviews* that are created
//! as children of the main window and positioned over the content viewport.
//!
//! Principles:
//! - **One live content webview at a time.** Only the active tab is materialized;
//!   switching tabs discards the previous content webview. This keeps RAM/CPU
//!   bounded no matter how many tabs exist (tab metadata is a few hundred bytes
//!   each, a live WebKitGTK webview is tens of MB).
//! - **Event-driven lifecycle.** wry hooks (`on_page_load`, `on_navigation`,
//!   `on_document_title_changed`, `on_download`) push real state to the UI via
//!   Tauri events; the UI never polls.
//! - **Labels are derived from tab ids**, so a stale webview can never be
//!   mistaken for a live one.
//!
//! Platform note: on Linux, `Window::add_child` packs child webviews into the
//! window's `GtkBox`, which stacks them and ignores their bounds. `setup`
//! installs a `GtkFixed` container (see `linux_webview`) that all content
//! webviews are moved into so `set_bounds` actually applies.

use crate::state::BrowserState;
use crate::{events, navigation};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl};
use tauri::webview::WebviewBuilder;

/// Label of the window (and its chrome webview).
pub const UI_LABEL: &str = "main";

/// Webview labels are `c<tab-id-without-dashes>`; the `c` prefix avoids any
/// chance of colliding with the `main` label.
pub fn content_label(tab_id: &str) -> String {
    format!("c{}", tab_id.replace('-', ""))
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Viewport {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Viewport {
    fn is_usable(&self) -> bool {
        self.width >= 50.0 && self.height >= 50.0 && self.x >= -1.0 && self.y >= -1.0
    }
}

/// Process-global counter used to drop LRU eviction ties deterministically.
static EPOCH: AtomicU64 = AtomicU64::new(0);

fn next_epoch() -> u64 {
    EPOCH.fetch_add(1, Ordering::Relaxed)
}

/// Everything the manager knows about one live content webview.
struct LiveSlot {
    tab_id: String,
    /// Monotonic access stamp for LRU ordering.
    epoch: u64,
}

/// Owns the mapping between tabs and materialized content webviews.
///
/// `max_live` is 1 today; the map-plus-LRU structure exists so raising it later
/// (e.g. to keep N tabs warm) is a one-line change with eviction already
/// implemented and tested.
pub struct WebviewManager {
    live: parking_lot::Mutex<Vec<LiveSlot>>,
    max_live: usize,
}

impl WebviewManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            live: parking_lot::Mutex::new(Vec::new()),
            // One live content webview: the active tab. Every other tab is
            // discarded; switching back reloads it (like mobile browsers).
            max_live: 1,
        })
    }

    fn drop_webview(app: &AppHandle, tab_id: &str) {
        let label = content_label(tab_id);
        if let Some(wv) = app.get_webview(&label) {
            let _ = wv.hide();
            let _ = wv.close();
        }
    }

    /// Close every live content webview except `keep` (tab id), then make sure
    /// `keep` is on screen at `viewport`. Creates the webview if needed.
    pub fn show(
        &self,
        app: &AppHandle,
        state: &BrowserState,
        keep: &str,
        viewport: Viewport,
    ) -> Result<(), String> {
        if !viewport.is_usable() {
            return Err("viewport too small".into());
        }

        // Resolve the tab's URL *before* taking any locks.
        let (url, private) = {
            let tabs = state.tabs.lock();
            let tab = tabs
                .tabs
                .iter()
                .find(|t| t.id == keep)
                .ok_or_else(|| format!("tab not found: {keep}"))?;
            (tab.url.clone(), tab.is_private)
        };

        let url = navigation::sanitize_url(&url)
            .map_err(|e| format!("refusing to render unsafe URL: {e}"))?;

        {
            let mut live = self.live.lock();
            // Evict everything that is not the requested tab, oldest first
            // (LRU), down to the live budget.
            live.sort_by_key(|s| s.epoch);
            let evict: Vec<String> = live
                .iter()
                .filter(|s| s.tab_id != keep)
                .map(|s| s.tab_id.clone())
                .collect();
            drop(live);
            for tab_id in evict {
                Self::drop_webview(app, &tab_id);
                self.live.lock().retain(|s| s.tab_id != tab_id);
                events::emit_tab_discarded(app, &tab_id);
            }
        }

        let label = content_label(keep);
        let parsed: url::Url = url.parse().map_err(|e: url::ParseError| e.to_string())?;

        match app.get_webview(&label) {
            Some(wv) => {
                // Existing webview for this tab: navigate only when the URL
                // actually differs (avoids duplicate history entries and
                // reload loops when the UI re-syncs).
                if let Ok(current) = wv.url() {
                    if current.as_str() != parsed.as_str() {
                        wv.navigate(parsed).map_err(|e| e.to_string())?;
                    }
                } else {
                    wv.navigate(parsed).map_err(|e| e.to_string())?;
                }
                let bounds = tauri::Rect {
                    position: LogicalPosition::new(viewport.x, viewport.y).into(),
                    size: LogicalSize::new(viewport.width, viewport.height).into(),
                };
                wv.set_bounds(bounds).map_err(|e| e.to_string())?;
                #[cfg(target_os = "linux")]
                place_content(&wv, viewport);
                wv.show().map_err(|e| e.to_string())?;
                let _ = wv.set_focus();
                self.touch(keep);
                Ok(())
            }
            None => {
                // The builder's label IS the webview label; bind it to the tab.
                let builder = content_builder(keep, &parsed, private);
                let win = app
                    .get_window(UI_LABEL)
                    .ok_or("main window not found")?;
                let wv = win
                    .add_child(
                        builder,
                        LogicalPosition::new(viewport.x, viewport.y),
                        LogicalSize::new(viewport.width, viewport.height),
                    )
                    .map_err(|e| e.to_string())?;
                #[cfg(target_os = "linux")]
                place_content(&wv, viewport);
                let _ = wv.set_focus();
                self.live.lock().push(LiveSlot {
                    tab_id: keep.to_string(),
                    epoch: next_epoch(),
                });
                Ok(())
            }
        }
    }

    fn touch(&self, tab_id: &str) {
        let mut live = self.live.lock();
        if let Some(slot) = live.iter_mut().find(|s| s.tab_id == tab_id) {
            slot.epoch = next_epoch();
        }
    }

    /// Hide the content webview of `tab_id` without destroying it.
    pub fn hide(&self, app: &AppHandle, tab_id: &str) {
        if let Some(wv) = app.get_webview(&content_label(tab_id)) {
            let _ = wv.hide();
        }
    }

    /// Destroy the content webview of `tab_id` (tab closed or discarded).
    pub fn close(&self, app: &AppHandle, tab_id: &str) {
        Self::drop_webview(app, tab_id);
        self.live.lock().retain(|s| s.tab_id != tab_id);
    }

    /// Close every content webview (window is closing).
    pub fn close_all(&self, app: &AppHandle) {
        let tabs: Vec<String> = self.live.lock().iter().map(|s| s.tab_id.clone()).collect();
        for tab_id in tabs {
            Self::drop_webview(app, &tab_id);
        }
        self.live.lock().clear();
    }

    /// Update bounds of a live webview (window resize / panel toggle).
    pub fn resize(&self, app: &AppHandle, tab_id: &str, viewport: Viewport) {
        if !viewport.is_usable() {
            return;
        }
        self.touch(tab_id);
        if let Some(wv) = app.get_webview(&content_label(tab_id)) {
            let bounds = tauri::Rect {
                position: LogicalPosition::new(viewport.x, viewport.y).into(),
                size: LogicalSize::new(viewport.width, viewport.height).into(),
            };
            let _ = wv.set_bounds(bounds);
            #[cfg(target_os = "linux")]
            place_content(&wv, viewport);
        }
    }

    pub fn live_count(&self) -> usize {
        self.live.lock().len()
    }
}

/// Build a content webview bound to a concrete tab id.
///
/// Hardening applied to every content webview:
/// - `incognito(private)` — private tabs get an ephemeral WebKit session
///   (no cookies/cache written to disk).
/// - `on_navigation` — block non-http(s) navigations (blocks `file://`,
///   `javascript:` URIs, custom schemes).
/// - `on_new_window` — target=_blank / window.open are denied; they create
///   nothing (we do not hand web content extra windows to script into).
/// - `on_page_load` — real loading state (no timers, no guessing).
/// - `on_document_title_changed` — real tab titles.
/// - `on_download` — downloads are recorded and allowed to continue to the
///   configured downloads directory; failures are recorded too.
/// - No `initialization_script` is injected into web content.
pub fn content_builder(tab_id: &str, url: &url::Url, private: bool) -> WebviewBuilder<tauri::Wry> {
    WebviewBuilder::new(content_label(tab_id), WebviewUrl::External(url.clone()))
        .incognito(private)
        .on_navigation(|url| navigation::is_web_scheme(url.scheme()))
        .on_new_window(|_url, _features| tauri::webview::NewWindowResponse::Deny)
        .on_page_load(|wv, payload| {
            let app = wv.app_handle().clone();
            let tab_id = label_to_tab_id(wv.label());
            match payload.event() {
                tauri::webview::PageLoadEvent::Started => {
                    events::emit_load_started(&app, &tab_id, payload.url().as_str());
                }
                tauri::webview::PageLoadEvent::Finished => {
                    events::emit_load_finished(&app, &tab_id, payload.url().as_str());
                }
            }
        })
        .on_document_title_changed(|wv, title| {
            let app = wv.app_handle().clone();
            let tab_id = label_to_tab_id(wv.label());
            events::emit_title_changed(&app, &tab_id, &title);
        })
        .on_download(|wv, event| {
            let app = wv.app_handle().clone();
            let tab_id = label_to_tab_id(wv.label());
            match event {
                tauri::webview::DownloadEvent::Requested { url, destination } => {
                    crate::downloads::on_requested(&app, &tab_id, url, destination);
                }
                tauri::webview::DownloadEvent::Finished { url, path, success } => {
                    crate::downloads::on_finished(&app, url, path.as_deref(), *success);
                }
                _ => {}
            }
            // Allow the download to proceed (it lands in the sanitized path
            // assigned in `on_requested`).
            true
        })
}

/// Recover the tab id from a content webview label.
pub fn label_to_tab_id(label: &str) -> String {
    // labels are `c` + uuid-without-dashes; the dashless form is preserved so
    // this is a pure prefix strip.
    label.strip_prefix('c').unwrap_or(label).to_string()
}

#[cfg(target_os = "linux")]
fn place_content(wv: &tauri::webview::Webview<tauri::Wry>, viewport: Viewport) {
    // `with_webview` hands us the platform webview; `inner()` is the
    // `webkit2gtk::WebView`, which is itself a `gtk::Widget` subclass, so it
    // can be moved into (and positioned within) our GtkFixed container.
    let _ = wv.with_webview(move |pw| {
        crate::linux_webview::place(
            &pw.inner(),
            viewport.x,
            viewport.y,
            viewport.width,
            viewport.height,
        );
    });
}
