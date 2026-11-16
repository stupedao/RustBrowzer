mod bookmarks;
mod browser;
mod database;
mod downloads;
mod engine;
mod extensions;
mod history;
mod navigation;
mod privacy;
mod profiles;
mod security;
mod settings;
mod tabs;

use browser::BrowserState;
use database::Database;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use settings::BrowserSettings;
use std::sync::Arc;
use tabs::Tab;
use tauri::{LogicalPosition, LogicalSize, Manager, State, WebviewUrl};
use tauri::webview::WebviewBuilder;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppSnapshot {
    tabs: tabs::TabState,
    settings: BrowserSettings,
}

#[tauri::command]
fn get_snapshot(state: State<'_, Arc<BrowserState>>) -> AppSnapshot {
    AppSnapshot {
        tabs: state.tabs.lock().clone(),
        settings: state.settings.lock().clone(),
    }
}

#[tauri::command]
fn new_tab(state: State<'_, Arc<BrowserState>>, url: Option<String>, is_private: Option<bool>) -> Tab {
    let private = is_private.unwrap_or(false);
    let homepage = state.settings.lock().homepage.clone();
    let tab = Tab::new(url.unwrap_or(homepage), private);
    let mut tabs = state.tabs.lock();
    let id = tab.id.clone();
    tabs.tabs.push(tab.clone());
    tabs.active_tab_id = Some(id);
    tab
}

#[tauri::command]
fn close_tab(state: State<'_, Arc<BrowserState>>, id: String) -> Option<String> {
    let mut tabs = state.tabs.lock();
    if let Some(pos) = tabs.tabs.iter().position(|t| t.id == id) {
        let removed = tabs.tabs.remove(pos);
        if !removed.is_private {
            tabs.closed_tabs.push(removed.clone());
            if tabs.closed_tabs.len() > 25 { tabs.closed_tabs.remove(0); }
            let _ = state.db.conn.lock().unwrap().execute(
                "INSERT INTO closed_tabs(url,title,closed_at) VALUES(?1,?2,datetime('now'))",
                rusqlite::params![removed.url, removed.title],
            );
        }
        if tabs.tabs.is_empty() {
            let home = state.settings.lock().homepage.clone();
            let t = Tab::new(home, removed.is_private);
            let nid = t.id.clone();
            tabs.tabs.push(t);
            tabs.active_tab_id = Some(nid.clone());
            return Some(nid);
        }
        let active = tabs.tabs.get(pos.saturating_sub(1).min(tabs.tabs.len()-1)).map(|t| t.id.clone());
        tabs.active_tab_id = active.clone();
        return active;
    }
    tabs.active_tab_id.clone()
}

#[tauri::command]
fn set_active_tab(state: State<'_, Arc<BrowserState>>, id: String) {
    let mut tabs = state.tabs.lock().ok();
    if let Some(ref mut tabs) = tabs {
        if tabs.tabs.iter().any(|t| t.id == id) {
            tabs.active_tab_id = Some(id);
        }
    }
}

#[tauri::command]
fn toggle_tab_pin(state: State<'_, Arc<BrowserState>>, id: String) -> Result<(), String> {
    state.toggle_tab_pin(id)
}

#[tauri::command]
fn toggle_tab_mute(state: State<'_, Arc<BrowserState>>, id: String) -> Result<(), String> {
    state.toggle_tab_mute(id)
}

#[tauri::command]
fn update_tab(state: State<'_, Arc<BrowserState>>, tab: Tab) {
    let mut tabs = state.tabs.lock().ok();
    if let Some(ref mut tabs) = tabs {
        if let Some(t) = tabs.tabs.iter_mut().find(|t| t.id == tab.id) {
            *t = tab;
        }
    }
}

#[tauri::command]
fn get_tab(state: State<'_, Arc<BrowserState>>, id: String) -> Result<Tab, String> {
    let tabs = state.tabs.lock().ok().ok_or_else(|| "Failed to lock tabs".to_string())?;
    tabs.tabs.iter()
        .find(|t| t.id == id)
        .cloned()
        .ok_or_else(|| format!("Tab not found: {}", id))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NavigatePayload { id: String, url: String, title: Option<String> }

#[tauri::command]
fn navigate_tab(state: State<'_, Arc<BrowserState>>, payload: NavigatePayload) {
    let mut tabs = state.tabs.lock();
    if let Some(t) = tabs.tabs.iter_mut().find(|t| t.id == payload.id) {
        t.url = payload.url.clone();
        if let Some(title) = payload.title { t.title = title; }
        t.loading = true;
        if !t.is_private {
            let _ = state.db.conn.lock().unwrap().execute(
                "INSERT INTO history(title,url,visited_at) VALUES(?1,?2,datetime('now'))",
                rusqlite::params![t.title, payload.url],
            );
        }
    }
}

#[tauri::command]
fn list_history(state: State<'_, Arc<BrowserState>>, limit: Option<i64>) -> Vec<serde_json::Value> {
    let conn = state.db.conn.lock().ok().unwrap_or_else(|| {
        Connection::open_in_memory().unwrap()
    });
    let lim = limit.unwrap_or(100); // Reduced from 200 to 100 for better performance
    let mut stmt = conn.prepare("SELECT id,title,url,visited_at FROM history ORDER BY visited_at DESC LIMIT ?1").unwrap();
    let rows = stmt.query_map(rusqlite::params![lim], |r| {
        Ok(serde_json::json!({"id": r.get::<_,i64>(0)?, "title": r.get::<_,Option<String>>(1)?, "url": r.get::<_,String>(2)?, "visitedAt": r.get::<_,String>(3)?}))
    }).unwrap();
    rows.filter_map(Result::ok).collect()
}

#[tauri::command]
fn clear_history(state: State<'_, Arc<BrowserState>>) -> Result<(), String> {
    state.db.conn.lock().unwrap().execute("DELETE FROM history", []).map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Bookmark { id: String, title: String, url: String, folder: Option<String>, created_at: String }

#[tauri::command]
fn list_bookmarks(state: State<'_, Arc<BrowserState>>) -> Vec<Bookmark> {
    let conn = state.db.conn.lock().ok().unwrap_or_else(|| {
        Connection::open_in_memory().unwrap()
    });
    let mut stmt = conn.prepare("SELECT id,title,url,folder,created_at FROM bookmarks ORDER BY created_at DESC LIMIT 100").unwrap();
    let rows = stmt.query_map([], |r| Ok(Bookmark{ id:r.get(0)?, title:r.get(1)?, url:r.get(2)?, folder:r.get(3)?, created_at:r.get(4)? })).unwrap();
    rows.filter_map(Result::ok).collect()
}

#[tauri::command]
fn add_bookmark(state: State<'_, Arc<BrowserState>>, title: String, url: String, folder: Option<String>) -> Bookmark {
    let b = Bookmark { id: uuid::Uuid::new_v4().to_string(), title, url, folder, created_at: chrono::Utc::now().to_rfc3339() };
    let _ = state.db.conn.lock().unwrap().execute(
        "INSERT OR REPLACE INTO bookmarks(id,title,url,folder,created_at) VALUES(?1,?2,?3,?4,?5)",
        rusqlite::params![b.id, b.title, b.url, b.folder, b.created_at],
    );
    b
}

#[tauri::command]
fn remove_bookmark(state: State<'_, Arc<BrowserState>>, id: String) -> Result<(), String> {
    state.db.conn.lock().unwrap().execute("DELETE FROM bookmarks WHERE id=?1", rusqlite::params![id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn get_settings(state: State<'_, Arc<BrowserState>>) -> BrowserSettings { state.settings.lock().clone() }

#[tauri::command]
fn save_settings(state: State<'_, Arc<BrowserState>>, settings: BrowserSettings) -> Result<(), String> {
    let json = serde_json::to_string(&settings).map_err(|e| e.to_string())?;
    state.db.set_setting("app_settings", &json).map_err(|e| e.to_string())?;
    *state.settings.lock() = settings;
    Ok(())
}

#[tauri::command]
fn resolve_url(input: String, state: State<'_, Arc<BrowserState>>) -> String {
    let engine = state.settings.lock().search_engine.clone();
    navigation::resolve_input(&input, &engine)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Viewport { x: f64, y: f64, width: f64, height: f64 }

/// Ensure a content webview exists for the given tab and navigate/show it.
/// v1: best-effort using Tauri unstable multiwebview APIs for real rendering.
#[tauri::command]
fn open_content(app: tauri::AppHandle, tab_id: String, url: String, viewport: Viewport) -> Result<(), String> {
    let label = format!("tab-{}", tab_id.replace('-', ""));

    // Sanitize URL to prevent XSS
    let sanitized = navigation::sanitize_url(&url)
        .map_err(|e| format!("Invalid URL: {}", e))?;

    let parsed = sanitized.parse().map_err(|e: url::ParseError| e.to_string())?;
    let win = app.get_window("main").ok_or("main window not found")?;

    if let Some(wv) = app.get_webview(&label) {
        let _ = wv.navigate(parsed);
        let _ = wv.set_position(LogicalPosition::new(viewport.x, viewport.y));
        let _ = wv.set_size(LogicalSize::new(viewport.width, viewport.height));
        let _ = wv.show();
        return Ok(());
    }

    let builder = WebviewBuilder::new(&label, WebviewUrl::External(parsed))
        .auto_resize()
        .initialization_script(r#"
            // Content Security Policy
            const meta = document.createElement('meta');
            meta.setAttribute('http-equiv', 'Content-Security-Policy');
            meta.setAttribute('content', "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; font-src 'self' data:; connect-src 'self' https:; frame-src 'self' https:; object-src 'none'; base-uri 'self'; form-action 'self';");
            document.head.appendChild(meta);

            // Disable local file access
            const link = document.createElement('link');
            link.rel = 'prefetch';
            link.href = 'about:blank';
            document.head.appendChild(link);
        "#);
    let webview = win.add_child(builder, LogicalPosition::new(viewport.x, viewport.y), LogicalSize::new(viewport.width, viewport.height))
        .map_err(|e| e.to_string())?;
    let _ = webview.set_focus();
    Ok(())
}

#[tauri::command]
fn hide_content(app: tauri::AppHandle, tab_id: String) -> Result<(), String> {
    let label = format!("tab-{}", tab_id.replace('-', ""));
    if let Some(wv) = app.get_webview(&label) { let _ = wv.hide(); }
    Ok(())
}

#[tauri::command]
fn resize_content(app: tauri::AppHandle, tab_id: String, viewport: Viewport) -> Result<(), String> {
    let label = format!("tab-{}", tab_id.replace('-', ""));
    if let Some(wv) = app.get_webview(&label) {
        let _ = wv.set_position(LogicalPosition::new(viewport.x, viewport.y));
        let _ = wv.set_size(LogicalSize::new(viewport.width, viewport.height));
    }
    Ok(())
}

#[tauri::command]
fn close_content(app: tauri::AppHandle, tab_id: String) -> Result<(), String> {
    let label = format!("tab-{}", tab_id.replace('-', ""));
    if let Some(wv) = app.get_webview(&label) { let _ = wv.close(); }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let proj = ProjectDirs::from("com", "rustium", "RUSTIUM").expect("project dirs");
    let data_dir = proj.data_dir().to_path_buf();
    std::fs::create_dir_all(&data_dir).ok();
    let db = Database::open(data_dir.join("rustium.db")).expect("db");
    let settings: BrowserSettings = db
        .get_setting("app_settings")
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let state = BrowserState::new(db, settings);

    tauri::Builder::default()
        .manage(state)
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_snapshot, new_tab, close_tab, set_active_tab, update_tab, get_tab,
            list_history, clear_history, list_bookmarks, add_bookmark, remove_bookmark,
            get_settings, save_settings, resolve_url, open_content, hide_content, resize_content, close_content,
            toggle_tab_pin, toggle_tab_mute
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
