//! RustBrowzer — a Tauri 2 browser shell.
//!
//! Module map:
//! - `renderer`  — content webview lifecycle (the "rendering architecture")
//! - `navigation`— URL resolution/sanitization + search engines
//! - `state`     — tabs, per-tab history, settings
//! - `database`  — SQLite persistence (history/bookmarks/session/downloads)
//! - `downloads` — WebKit download pipeline
//! - `events`    — typed backend -> UI events
//! - `commands`  — the IPC surface (all guarded to only run for the UI webview)
//! - `linux_webview` — GtkFixed container so child webviews honor their bounds

mod commands;
mod database;
mod downloads;
mod events;
#[cfg(target_os = "linux")]
mod linux_webview;
mod navigation;
mod renderer;
mod state;

use directories::ProjectDirs;
use state::{BrowserSettings, BrowserState};
use std::sync::Arc;

/// Choose default directories without baking branding into the UI.
fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("", "", "RustBrowzer")
}

pub fn run() {
    let proj = project_dirs().expect("project dirs");
    let data_dir = proj.data_dir().to_path_buf();
    let _ = std::fs::create_dir_all(&data_dir);
    let db = database::Database::open(data_dir.join("browser.db")).expect("open database");

    // ---- load settings (with legacy migration) ----
    let mut settings: BrowserSettings = db
        .get_setting("app_settings")
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    // Migrate the old single-prefix engine setting to the named-engine model.
    let mut custom_engines = Vec::new();
    if let Ok(old) = serde_json::from_str::<OldSettings>(
        &db.get_setting("app_settings").unwrap_or_default(),
    ) {
        if let Some(prefix) = old.search_engine {
            // Only keep it if it is not one of the builtins.
            let builtins = navigation::builtin_engines();
            let known = builtins.iter().any(|e| e.build_query("{}") == prefix || {
                // fuzzy: compare the query prefix shape
                prefix.starts_with(&e.template.replace("{}", ""))
            });
            if !known && prefix.starts_with("https://") {
                custom_engines.push(navigation::SearchEngine {
                    name: "Custom".into(),
                    keyword: "c".into(),
                    template: prefix.replace("q={}", "{}").replace("%s", "{}"),
                    suggest_url: None,
                });
            }
        }
    }
    // One-time upgrade from the stock homepage to the built-in start page.
    if settings.homepage.trim().is_empty()
        || settings.homepage == "https://example.com"
        || settings.homepage == "rustium://home"
    {
        settings.homepage = navigation::HOME_URL.into();
    }

    // ---- session restore ----
    let restore: Option<SessionRestore> = db
        .load_session()
        .and_then(|s| serde_json::from_str(&s).ok());
    let state = BrowserState::new(db, settings, custom_engines);

    if let Some(session) = restore {
        let mut tabs = state.tabs.lock();
        if !session.tabs.is_empty() {
            tabs.tabs = session.tabs;
            tabs.active_tab_id = Some(
                session
                    .active_tab_id
                    .filter(|id| session.tabs.iter().any(|t| t.id == *id))
                    .unwrap_or_else(|| session.tabs[0].id.clone()),
            );
        }
    }

    tauri::Builder::default()
        .manage(state)
        .manage(renderer::WebviewManager::new())
        .setup(|app| {
            #[cfg(target_os = "linux")]
            {
                if let Some(win) = app.get_webview_window(renderer::UI_LABEL) {
                    match win.default_vbox() {
                        Ok(vbox) => linux_webview::install(&vbox),
                        Err(e) => eprintln!("[browser] linux webview layout unavailable: {e}"),
                    }
                    // Keyboard accelerators that work even while web content
                    // has keyboard focus (Linux/GTK).
                    linux_webview::install_accelerators(&win, app.handle().clone());
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // snapshot / tabs
            commands::get_snapshot,
            commands::new_tab,
            commands::close_tab,
            commands::select_tab,
            commands::navigate,
            commands::go_back,
            commands::go_forward,
            commands::reload,
            commands::stop,
            commands::toggle_tab_pin,
            commands::toggle_tab_mute,
            commands::move_tab,
            commands::reopen_tab,
            // viewport
            commands::content_ready,
            commands::update_viewport,
            // omnibox
            commands::omnibox_suggest,
            // history / bookmarks / downloads
            commands::list_history,
            commands::search_history,
            commands::clear_history,
            commands::list_bookmarks,
            commands::add_bookmark,
            commands::remove_bookmark,
            commands::is_bookmarked,
            commands::list_downloads,
            // settings
            commands::get_settings,
            commands::save_settings,
            commands::list_engines,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                // Persist the session before the webviews are torn down.
                if let Some(state) = window.try_state::<BrowserState>() {
                    let tabs_json = {
                        let tabs = state.tabs.lock();
                        serde_json::to_string(&SessionRestore {
                            tabs: tabs.tabs.clone(),
                            active_tab_id: tabs.active_tab_id.clone(),
                        })
                        .unwrap_or_else(|_| "{}".into())
                    };
                    let _ = state.db.save_session(&tabs_json);
                }
                if let Some(mgr) = window.try_state::<Arc<renderer::WebviewManager>>() {
                    let app = window.app_handle().clone();
                    mgr.close_all(&app);
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[derive(serde::Deserialize)]
struct OldSettings {
    #[serde(default)]
    search_engine: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SessionRestore {
    tabs: Vec<state::Tab>,
    active_tab_id: Option<String>,
}
