//! Typed backend -> UI events.
//!
//! All events are emitted **to the `main` webview only** (`emit_to`). Content
//! webviews are remote origins; they get no IPC capability (see
//! `capabilities/default.json`), and events never target them either, so
//! browsing data (titles, URLs) cannot leak into web content.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::renderer::UI_LABEL;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabSnapshot {
    pub id: String,
    pub title: String,
    pub url: String,
    pub loading: bool,
    pub pinned: bool,
    pub muted: bool,
    pub is_private: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabsChanged {
    pub tabs: Vec<TabSnapshot>,
    pub active_tab_id: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadSnapshot {
    pub id: String,
    pub url: String,
    pub filename: String,
    pub path: String,
    pub status: String,
    pub total_bytes: Option<i64>,
    pub received_bytes: i64,
}

fn send<T: Serialize + Clone>(app: &AppHandle, event: &str, payload: T) {
    // UI_LABEL is both the window label and its webview label.
    if let Err(e) = app.emit_to(UI_LABEL, event, payload) {
        eprintln!("[browser] emit {event} failed: {e}");
    }
}

pub fn emit_tabs_changed(app: &AppHandle, payload: TabsChanged) {
    send(app, "tabs-changed", payload);
}

pub fn emit_load_started(app: &AppHandle, tab_id: &str, url: &str) {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct P<'a> {
        tab_id: &'a str,
        url: &'a str,
    }
    send(app, "load-started", P { tab_id, url });
}

pub fn emit_load_finished(app: &AppHandle, tab_id: &str, url: &str) {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct P<'a> {
        tab_id: &'a str,
        url: &'a str,
    }
    send(app, "load-finished", P { tab_id, url });
}

pub fn emit_title_changed(app: &AppHandle, tab_id: &str, title: &str) {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct P<'a> {
        tab_id: &'a str,
        title: &'a str,
    }
    send(app, "title-changed", P { tab_id, title });
}

pub fn emit_tab_discarded(app: &AppHandle, tab_id: &str) {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct P<'a> {
        tab_id: &'a str,
    }
    send(app, "tab-discarded", P { tab_id });
}

pub fn emit_url_changed(app: &AppHandle, tab_id: &str, url: &str) {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct P<'a> {
        tab_id: &'a str,
        url: &'a str,
    }
    send(app, "url-changed", P { tab_id, url });
}

pub fn emit_downloads_changed(app: &AppHandle, downloads: Vec<DownloadSnapshot>) {
    send(app, "downloads-changed", downloads);
}
