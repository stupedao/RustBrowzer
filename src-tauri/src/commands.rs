//! The IPC command surface.
//!
//! Every command goes through [`assert_ui_origin`], which rejects calls that
//! did not originate from the browser UI webview. This is defense in depth on
//! top of the Tauri ACL: the `default` capability only grants `core:default`
//! to the `main` webview, and content webviews are remote origins that
//! resolve to no permissions at all — but a second, explicit check means a
//! future capability mistake cannot expose the browser's internals to web
//! pages.

use crate::database::{BookmarkRow, DownloadRow, HistoryEntry};
use crate::events::TabSnapshot;
use crate::navigation::{self, SearchEngine};
use crate::renderer::{self, Viewport, WebviewManager};
use crate::state::{BrowserSettings, BrowserState, Tab, TabState};
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State, Webview};

const ERR_NOT_UI: &str = "command refused: caller is not the browser UI";

/// Reject IPC calls from anything but the UI webview.
fn assert_ui_origin(webview: &Webview<tauri::Wry>) -> Result<(), String> {
    if webview.label() == renderer::UI_LABEL {
        Ok(())
    } else {
        Err(ERR_NOT_UI.into())
    }
}

/// Resolve omnibox-style input using the current settings.
fn resolve_input(state: &Arc<BrowserState>, raw: &str) -> String {
    let s = state.settings.lock();
    if navigation::is_internal_url(raw) {
        navigation::HOME_URL.into()
    } else {
        navigation::resolve_input(raw, &s.engines, &s.settings.default_engine)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub tabs: Vec<TabSnapshot>,
    pub active_tab_id: Option<String>,
    pub settings: BrowserSettings,
    pub engines: Vec<SearchEngine>,
}

#[tauri::command]
pub fn get_snapshot(
    webview: Webview<tauri::Wry>,
    state: State<'_, Arc<BrowserState>>,
) -> Result<Snapshot, String> {
    assert_ui_origin(&webview)?;
    let tabs = state.tabs.lock();
    let s = state.settings.lock();
    Ok(Snapshot {
        tabs: tabs.tabs.iter().map(TabSnapshot::from).collect(),
        active_tab_id: tabs.active_tab_id.clone(),
        settings: s.settings.clone(),
        engines: s.engines.clone(),
    })
}

#[tauri::command]
pub fn new_tab(
    webview: Webview<tauri::Wry>,
    app: AppHandle,
    state: State<'_, Arc<BrowserState>>,
    url: Option<String>,
    is_private: Option<bool>,
    background: Option<bool>,
) -> Result<Tab, String> {
    assert_ui_origin(&webview)?;
    let private = is_private.unwrap_or(false);
    let make_active = !background.unwrap_or(false);
    let homepage = state.settings.lock().settings.homepage.clone();
    let raw = url.unwrap_or(homepage);
    let tab = Tab::new(resolve_input(&state, &raw), private);
    let returned = tab.clone();
    state.with_tabs(&app, |tabs: &mut TabState| {
        let id = tab.id.clone();
        // Insert after the active tab (like real browsers).
        let pos = tabs
            .tabs
            .iter()
            .position(|t| Some(&t.id) == tabs.active_tab_id.as_ref())
            .map(|p| p + 1)
            .unwrap_or(tabs.tabs.len());
        tabs.tabs.insert(pos, tab.clone());
        if make_active {
            tabs.active_tab_id = Some(id);
        }
    });
    // The UI materializes the webview via `content_ready` when it receives
    // `tabs-changed`; nothing else to do here.
    Ok(returned)
}

#[tauri::command]
pub fn close_tab(
    webview: Webview<tauri::Wry>,
    app: AppHandle,
    state: State<'_, Arc<BrowserState>>,
    mgr: State<'_, Arc<WebviewManager>>,
    id: String,
) -> Result<Option<String>, String> {
    assert_ui_origin(&webview)?;
    let active_now = state.tabs.lock().active_tab_id.clone();
    let next_active = state.with_tabs(&app, |tabs| {
        let pos = tabs.tabs.iter().position(|t| t.id == id)?;
        let removed = tabs.tabs.remove(pos);
        tabs.history.remove(&id);
        if tabs.tabs.is_empty() {
            // Always keep one tab open.
            let home = state.settings.lock().settings.homepage.clone();
            let t = Tab::new(home, removed.is_private);
            let nid = t.id.clone();
            tabs.tabs.push(t);
            tabs.active_tab_id = Some(nid.clone());
            return Some(nid);
        }
        // Choose the neighbor the way real browsers do: closing the active
        // tab focuses its right neighbor (else the left one); closing a
        // background tab leaves the active tab alone.
        if active_now.as_deref() == Some(id.as_str()) {
            let neighbor = tabs
                .tabs
                .get(pos)
                .or_else(|| tabs.tabs.get(pos.saturating_sub(1)))
                .map(|t| t.id.clone())
                .unwrap_or_else(|| tabs.tabs[0].id.clone());
            tabs.active_tab_id = Some(neighbor.clone());
            Some(neighbor)
        } else {
            None
        }
    });
    mgr.close(&app, &id);
    Ok(next_active)
}

#[tauri::command]
pub fn select_tab(
    webview: Webview<tauri::Wry>,
    app: AppHandle,
    state: State<'_, Arc<BrowserState>>,
    id: String,
) -> Result<(), String> {
    assert_ui_origin(&webview)?;
    state.with_tabs(&app, |tabs| {
        if tabs.tabs.iter().any(|t| t.id == id) {
            tabs.active_tab_id = Some(id);
        }
    });
    Ok(())
}

/// Navigate the given (or active) tab to `input` (omnibox rules apply).
#[tauri::command]
pub fn navigate(
    webview: Webview<tauri::Wry>,
    app: AppHandle,
    state: State<'_, Arc<BrowserState>>,
    id: Option<String>,
    input: String,
    viewport: Option<Viewport>,
) -> Result<String, String> {
    assert_ui_origin(&webview)?;
    let tab_id = id
        .or_else(|| state.active_tab().map(|t| t.id))
        .ok_or("no tab to navigate")?;

    let url = {
        let s = state.settings.lock();
        navigation::resolve_input(&input, &s.engines, &s.settings.default_engine)
    };

    let is_internal = navigation::is_internal_url(&url);
    let resolved = state.with_tabs(&app, |tabs| -> Result<String, String> {
        let previous_url = {
            let tab = tabs
                .tabs
                .iter()
                .find(|t| t.id == tab_id)
                .ok_or_else(|| format!("tab not found: {tab_id}"))?;
            tab.url.clone()
        };
        if !is_internal {
            state.push_history(tabs, &tab_id, &previous_url);
        }
        let (url, title) = {
            let tab = tabs.tabs.iter_mut().find(|t| t.id == tab_id).unwrap();
            tab.url = url.clone();
            if is_internal {
                tab.title = "New Tab".into();
                tab.loading = false;
            } else {
                tab.title = navigation::pretty_host(&url);
                tab.loading = true;
            }
            (tab.url.clone(), tab.title.clone())
        };
        let private = tabs
            .tabs
            .iter()
            .find(|t| t.id == tab_id)
            .map(|t| t.is_private)
            .unwrap_or(false);
        if !private && !is_internal {
            state.db.add_history(&title, &url);
        }
        Ok(url)
    })?;

    if is_internal {
        // The start page is drawn by the UI; close any content webview so it
        // cannot cover the page.
        if let Some(mgr) = app.try_state::<Arc<WebviewManager>>() {
            mgr.close(&app, &tab_id);
        }
    } else if let Some(vp) = viewport {
        if let Some(mgr) = app.try_state::<Arc<WebviewManager>>() {
            mgr.show(&app, &state, &tab_id, vp)?;
        }
    }
    Ok(resolved)
}

#[tauri::command]
pub fn go_back(
    webview: Webview<tauri::Wry>,
    app: AppHandle,
    state: State<'_, Arc<BrowserState>>,
    id: Option<String>,
    viewport: Option<Viewport>,
) -> Result<bool, String> {
    assert_ui_origin(&webview)?;
    let tab_id = id
        .or_else(|| state.active_tab().map(|t| t.id))
        .ok_or("no tab")?;
    {
        let mut tabs = state.tabs.lock();
        let Some(entry) = tabs.history.get_mut(&tab_id) else {
            return Ok(false);
        };
        let Some(url) = entry.back.pop() else {
            return Ok(false);
        };
        let Some(tab) = tabs.tabs.iter_mut().find(|t| t.id == tab_id) else {
            return Ok(false);
        };
        entry.forward.push(tab.url.clone());
        tab.url = url.clone();
        tab.loading = true;
        tab.can_go_back = !entry.back.is_empty();
        tab.can_go_forward = true;
        let private = tab.is_private;
        let title = tab.title.clone();
        if !private {
            state.db.add_history(&title, &url);
        }
        url
    };
    let snapshot = state.tabs.lock().snapshot_public();
    crate::events::emit_tabs_changed(&app, snapshot);
    if let Some(vp) = viewport {
        if let Some(mgr) = app.try_state::<Arc<WebviewManager>>() {
            mgr.show(&app, &state, &tab_id, vp)?;
        }
    }
    Ok(true)
}

#[tauri::command]
pub fn go_forward(
    webview: Webview<tauri::Wry>,
    app: AppHandle,
    state: State<'_, Arc<BrowserState>>,
    id: Option<String>,
    viewport: Option<Viewport>,
) -> Result<bool, String> {
    assert_ui_origin(&webview)?;
    let tab_id = id
        .or_else(|| state.active_tab().map(|t| t.id))
        .ok_or("no tab")?;
    {
        let mut tabs = state.tabs.lock();
        let Some(entry) = tabs.history.get_mut(&tab_id) else {
            return Ok(false);
        };
        let Some(url) = entry.forward.pop() else {
            return Ok(false);
        };
        let Some(tab) = tabs.tabs.iter_mut().find(|t| t.id == tab_id) else {
            return Ok(false);
        };
        entry.back.push(tab.url.clone());
        tab.url = url.clone();
        tab.loading = true;
        tab.can_go_forward = !entry.forward.is_empty();
        tab.can_go_back = true;
        let private = tab.is_private;
        let title = tab.title.clone();
        if !private {
            state.db.add_history(&title, &url);
        }
        url
    };
    let snapshot = state.tabs.lock().snapshot_public();
    crate::events::emit_tabs_changed(&app, snapshot);
    if let Some(vp) = viewport {
        if let Some(mgr) = app.try_state::<Arc<WebviewManager>>() {
            mgr.show(&app, &state, &tab_id, vp)?;
        }
    }
    Ok(true)
}

#[tauri::command]
pub fn reload(
    webview: Webview<tauri::Wry>,
    app: AppHandle,
    state: State<'_, Arc<BrowserState>>,
    id: Option<String>,
    viewport: Option<Viewport>,
) -> Result<(), String> {
    assert_ui_origin(&webview)?;
    let tab = {
        let tabs = state.tabs.lock();
        let tab_id = id
            .or_else(|| tabs.active_tab_id.clone())
            .ok_or("no tab")?;
        tabs.tabs.iter().find(|t| t.id == tab_id).cloned()
    }
    .ok_or("tab not found")?;

    if navigation::is_internal_url(&tab.url) {
        return Ok(()); // start page: nothing to reload
    }
    state.with_tabs(&app, |tabs| {
        if let Some(t) = tabs.tabs.iter_mut().find(|t| t.id == tab.id) {
            t.loading = true;
        }
    });
    if let Some(wv) = app.get_webview(&renderer::content_label(&tab.id)) {
        let _ = wv.reload();
        return Ok(());
    }
    if let Some(vp) = viewport {
        if let Some(mgr) = app.try_state::<Arc<WebviewManager>>() {
            mgr.show(&app, &state, &tab.id, vp)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn stop(
    webview: Webview<tauri::Wry>,
    app: AppHandle,
    state: State<'_, Arc<BrowserState>>,
    id: Option<String>,
) -> Result<(), String> {
    assert_ui_origin(&webview)?;
    let tab_id = id
        .or_else(|| state.active_tab().map(|t| t.id))
        .ok_or("no tab")?;
    // wry exposes no stop-load API; mark not-loading and let the next page
    // event correct it. Honest limitation.
    state.with_tabs(&app, |tabs| {
        if let Some(t) = tabs.tabs.iter_mut().find(|t| t.id == tab_id) {
            t.loading = false;
        }
    });
    Ok(())
}

#[tauri::command]
pub fn toggle_tab_pin(
    webview: Webview<tauri::Wry>,
    app: AppHandle,
    state: State<'_, Arc<BrowserState>>,
    id: String,
) -> Result<bool, String> {
    assert_ui_origin(&webview)?;
    Ok(state.with_tabs(&app, |tabs| {
        let t = tabs
            .tabs
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| format!("tab not found: {id}"))?;
        t.pinned = !t.pinned;
        let pinned = t.pinned;
        // Keep pinned tabs sorted to the front (stable sort keeps order).
        tabs.tabs.sort_by_key(|t| !t.pinned);
        pinned
    }))
}

#[tauri::command]
pub fn toggle_tab_mute(
    webview: Webview<tauri::Wry>,
    app: AppHandle,
    state: State<'_, Arc<BrowserState>>,
    id: String,
) -> Result<bool, String> {
    assert_ui_origin(&webview)?;
    // WebKitGTK does not expose per-webview mute through wry; this toggles
    // the browser-side flag the UI displays. Honest limitation.
    Ok(state.with_tabs(&app, |tabs| {
        let t = tabs
            .tabs
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| format!("tab not found: {id}"))?;
        t.muted = !t.muted;
        t.muted
    }))
}

/// Move a tab to a new index (drag & drop in the tab strip).
#[tauri::command]
pub fn move_tab(
    webview: Webview<tauri::Wry>,
    app: AppHandle,
    state: State<'_, Arc<BrowserState>>,
    id: String,
    to_index: usize,
) -> Result<(), String> {
    assert_ui_origin(&webview)?;
    state.with_tabs(&app, |tabs| -> Result<(), String> {
        let from = tabs
            .tabs
            .iter()
            .position(|t| t.id == id)
            .ok_or_else(|| format!("tab not found: {id}"))?;
        if tabs.tabs.is_empty() {
            return Ok(());
        }
        let to = to_index.min(tabs.tabs.len() - 1);
        let tab = tabs.tabs.remove(from);
        tabs.tabs.insert(to, tab);
        // Pinned tabs stay left of unpinned ones.
        tabs.tabs.sort_by_key(|t| !t.pinned);
        Ok(())
    })
}

/// Reopen a closed tab (Ctrl+Shift+T). `url` comes from the UI's closed-tabs
/// stack (kept client-side, capped, never persisted for private tabs).
#[tauri::command]
pub fn reopen_tab(
    webview: Webview<tauri::Wry>,
    app: AppHandle,
    state: State<'_, Arc<BrowserState>>,
    url: Option<String>,
    is_private: Option<bool>,
) -> Result<Option<Tab>, String> {
    assert_ui_origin(&webview)?;
    let url = match url {
        Some(u) => u,
        None => return Ok(None),
    };
    let homepage_if_empty = state.settings.lock().settings.homepage.clone();
    let target = if url.trim().is_empty() {
        homepage_if_empty
    } else {
        url
    };
    let tab = Tab::new(resolve_input(&state, &target), is_private.unwrap_or(false));
    let returned = tab.clone();
    state.with_tabs(&app, |tabs| {
        let id = tab.id.clone();
        tabs.tabs.push(tab.clone());
        tabs.active_tab_id = Some(id);
    });
    Ok(Some(returned))
}

/// The UI reports its content viewport and asks for the active tab to be
/// materialized there (called on mount, tab switch, layout change).
#[tauri::command]
pub fn content_ready(
    webview: Webview<tauri::Wry>,
    app: AppHandle,
    state: State<'_, Arc<BrowserState>>,
    mgr: State<'_, Arc<WebviewManager>>,
    viewport: Viewport,
) -> Result<(), String> {
    assert_ui_origin(&webview)?;
    let tab = state.active_tab().ok_or("no active tab")?;
    if navigation::is_internal_url(&tab.url) {
        mgr.close(&app, &tab.id);
        return Ok(());
    }
    mgr.show(&app, &state, &tab.id, viewport)
}

/// Bounds of the content viewport changed (resize, panel toggle, fullscreen).
#[tauri::command]
pub fn update_viewport(
    webview: Webview<tauri::Wry>,
    app: AppHandle,
    state: State<'_, Arc<BrowserState>>,
    mgr: State<'_, Arc<WebviewManager>>,
    viewport: Viewport,
) -> Result<(), String> {
    assert_ui_origin(&webview)?;
    let tab = state.active_tab().ok_or("no active tab")?;
    if !navigation::is_internal_url(&tab.url) {
        mgr.resize(&app, &tab.id, viewport);
    }
    Ok(())
}

// ---- omnibox suggestions ----

#[derive(Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Suggestion {
    #[serde(rename_all = "camelCase")]
    OpenTab { title: String, url: String, tab_id: String },
    #[serde(rename_all = "camelCase")]
    Bookmark { title: String, url: String },
    #[serde(rename_all = "camelCase")]
    History { title: String, url: String, visit_count: i64 },
    #[serde(rename_all = "camelCase")]
    Search { query: String, engine: String, url: String },
}

/// Omnibox suggestions: open tabs, bookmarks, frecency history, and a default
/// search row. Never triggers network requests (privacy + speed).
#[tauri::command]
pub fn omnibox_suggest(
    webview: Webview<tauri::Wry>,
    state: State<'_, Arc<BrowserState>>,
    input: String,
) -> Result<Vec<Suggestion>, String> {
    assert_ui_origin(&webview)?;
    let trimmed = input.trim();
    let mut out = Vec::new();
    if trimmed.is_empty() {
        return Ok(out);
    }
    let lower = trimmed.to_lowercase();

    // Open tabs matching title or URL (max 3).
    {
        let tabs = state.tabs.lock();
        for t in tabs.tabs.iter() {
            if navigation::is_internal_url(&t.url) {
                continue;
            }
            if t.title.to_lowercase().contains(&lower) || t.url.to_lowercase().contains(&lower) {
                out.push(Suggestion::OpenTab {
                    title: t.title.clone(),
                    url: t.url.clone(),
                    tab_id: t.id.clone(),
                });
            }
            if out.len() >= 3 {
                break;
            }
        }
    }

    // Bookmarks (up to 6 total).
    for b in state.db.list_bookmarks() {
        if b.title.to_lowercase().contains(&lower) || b.url.to_lowercase().contains(&lower) {
            out.push(Suggestion::Bookmark {
                title: b.title,
                url: b.url,
            });
        }
        if out.len() >= 6 {
            break;
        }
    }

    // History, frecency-ordered (up to 12 total).
    for h in state.db.search_history(&lower, 8) {
        out.push(Suggestion::History {
            title: h.title.unwrap_or_else(|| navigation::pretty_host(&h.url)),
            url: h.url,
            visit_count: h.visit_count,
        });
        if out.len() >= 12 {
            break;
        }
    }

    // Trailing "search for …" row using the default engine.
    {
        let s = state.settings.lock();
        let engine = s.default_engine().clone();
        out.push(Suggestion::Search {
            query: trimmed.to_string(),
            engine: engine.name.clone(),
            url: engine.build_query(trimmed),
        });
    }

    Ok(out)
}

// ---- history / bookmarks / downloads ----

#[tauri::command]
pub fn list_history(
    webview: Webview<tauri::Wry>,
    state: State<'_, Arc<BrowserState>>,
    limit: Option<i64>,
) -> Result<Vec<HistoryEntry>, String> {
    assert_ui_origin(&webview)?;
    Ok(state.db.list_history(limit.unwrap_or(100).clamp(1, 500)))
}

#[tauri::command]
pub fn search_history(
    webview: Webview<tauri::Wry>,
    state: State<'_, Arc<BrowserState>>,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<HistoryEntry>, String> {
    assert_ui_origin(&webview)?;
    Ok(state.db.search_history(&query, limit.unwrap_or(50).clamp(1, 200)))
}

#[tauri::command]
pub fn clear_history(
    webview: Webview<tauri::Wry>,
    state: State<'_, Arc<BrowserState>>,
) -> Result<(), String> {
    assert_ui_origin(&webview)?;
    state.db.clear_history().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_bookmarks(
    webview: Webview<tauri::Wry>,
    state: State<'_, Arc<BrowserState>>,
) -> Result<Vec<BookmarkRow>, String> {
    assert_ui_origin(&webview)?;
    Ok(state.db.list_bookmarks())
}

#[tauri::command]
pub fn add_bookmark(
    webview: Webview<tauri::Wry>,
    state: State<'_, Arc<BrowserState>>,
    title: String,
    url: String,
    folder: Option<String>,
) -> Result<String, String> {
    assert_ui_origin(&webview)?;
    let clean_url =
        navigation::sanitize_url(&url).map_err(|e| format!("cannot bookmark: {e}"))?;
    state
        .db
        .add_bookmark(&title, &clean_url, folder.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_bookmark(
    webview: Webview<tauri::Wry>,
    state: State<'_, Arc<BrowserState>>,
    id: String,
) -> Result<(), String> {
    assert_ui_origin(&webview)?;
    state.db.remove_bookmark(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn is_bookmarked(
    webview: Webview<tauri::Wry>,
    state: State<'_, Arc<BrowserState>>,
    url: String,
) -> Result<bool, String> {
    assert_ui_origin(&webview)?;
    Ok(state.db.is_bookmarked(&url))
}

#[tauri::command]
pub fn list_downloads(
    webview: Webview<tauri::Wry>,
    state: State<'_, Arc<BrowserState>>,
) -> Result<Vec<DownloadRow>, String> {
    assert_ui_origin(&webview)?;
    Ok(state.db.list_downloads())
}

// ---- settings ----

#[tauri::command]
pub fn get_settings(
    webview: Webview<tauri::Wry>,
    state: State<'_, Arc<BrowserState>>,
) -> Result<BrowserSettings, String> {
    assert_ui_origin(&webview)?;
    Ok(state.settings.lock().settings.clone())
}

#[tauri::command]
pub fn save_settings(
    webview: Webview<tauri::Wry>,
    state: State<'_, Arc<BrowserState>>,
    settings: BrowserSettings,
) -> Result<(), String> {
    assert_ui_origin(&webview)?;
    let mut s = state.settings.lock();
    if !s
        .engines
        .iter()
        .any(|e| e.name.eq_ignore_ascii_case(&settings.default_engine))
    {
        return Err(format!("unknown search engine: {}", settings.default_engine));
    }
    let homepage = if navigation::is_internal_url(&settings.homepage) {
        navigation::HOME_URL.to_string()
    } else {
        navigation::sanitize_url(&settings.homepage)
            .map_err(|e| format!("invalid homepage: {e}"))?
    };
    let mut next = settings;
    next.homepage = homepage;
    s.settings = next.clone();
    let json = serde_json::to_string(&next).map_err(|e| e.to_string())?;
    state
        .db
        .set_setting("app_settings", &json)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn list_engines(
    webview: Webview<tauri::Wry>,
    state: State<'_, Arc<BrowserState>>,
) -> Result<Vec<SearchEngine>, String> {
    assert_ui_origin(&webview)?;
    Ok(state.settings.lock().engines.clone())
}

impl TabState {
    /// Snapshot for events emitted outside `with_tabs`.
    pub fn snapshot_public(&self) -> crate::events::TabsChanged {
        crate::events::TabsChanged {
            tabs: self.tabs.iter().map(TabSnapshot::from).collect(),
            active_tab_id: self.active_tab_id.clone(),
        }
    }
}
