//! Shared browser state (tabs, per-tab history stacks, settings) and the
//! helpers that mutate it. Every mutation funnels through
//! [`BrowserState::with_tabs`] so the UI is always told about the change.

use crate::database::Database;
use crate::events::{self, TabSnapshot, TabsChanged};
use crate::navigation::{self, SearchEngine};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::AppHandle;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tab {
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

impl Tab {
    pub fn new(url: String, is_private: bool) -> Self {
        let title = if navigation::is_internal_url(&url) {
            "New Tab".to_string()
        } else {
            navigation::pretty_host(&url)
        };
        Self {
            id: Uuid::new_v4().to_string(),
            title,
            url,
            loading: false,
            pinned: false,
            muted: false,
            is_private,
            can_go_back: false,
            can_go_forward: false,
        }
    }
}

/// Navigation history for one tab (the webview itself is discarded when the
/// tab is inactive, so we own the back/forward stacks ourselves).
#[derive(Debug, Clone, Default)]
pub struct NavHistory {
    pub back: Vec<String>,
    pub forward: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct TabState {
    pub tabs: Vec<Tab>,
    pub active_tab_id: Option<String>,
    /// back/forward stacks keyed by tab id (entries are URLs).
    pub history: std::collections::HashMap<String, NavHistory>,
}

impl From<&Tab> for TabSnapshot {
    fn from(t: &Tab) -> Self {
        Self {
            id: t.id.clone(),
            title: t.title.clone(),
            url: t.url.clone(),
            loading: t.loading,
            pinned: t.pinned,
            muted: t.muted,
            is_private: t.is_private,
            can_go_back: t.can_go_back,
            can_go_forward: t.can_go_forward,
        }
    }
}

impl TabState {
    fn snapshot(&self) -> TabsChanged {
        TabsChanged {
            tabs: self.tabs.iter().map(TabSnapshot::from).collect(),
            active_tab_id: self.active_tab_id.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserSettings {
    pub theme: String,
    /// Name of the default search engine (must exist in `engines`).
    pub default_engine: String,
    pub homepage: String,
    pub download_dir: Option<String>,
    pub startup_behavior: String,
    pub show_bookmarks_bar: bool,
    pub block_popups: bool,
}

impl Default for BrowserSettings {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            default_engine: "DuckDuckGo".into(),
            homepage: navigation::HOME_URL.into(),
            download_dir: None,
            startup_behavior: "homepage".into(),
            show_bookmarks_bar: true,
            block_popups: true,
        }
    }
}

pub struct BrowserState {
    pub db: Database,
    pub tabs: Mutex<TabState>,
    pub settings: Mutex<SettingsWithEngines>,
}

/// Settings plus the resolved engine list (builtin engines + saved custom
/// ones), kept together so the omnibox always sees a consistent pair.
#[derive(Debug, Clone)]
pub struct SettingsWithEngines {
    pub settings: BrowserSettings,
    pub engines: Vec<SearchEngine>,
}

impl SettingsWithEngines {
    pub fn default_engine(&self) -> &SearchEngine {
        self.engines
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(&self.settings.default_engine))
            .or_else(|| self.engines.first())
            .expect("engine list is never empty")
    }
}

impl BrowserState {
    pub fn new(db: Database, settings: BrowserSettings, custom_engines: Vec<SearchEngine>) -> Arc<Self> {
        let mut tabs = TabState::default();
        let home = settings.homepage.clone();
        let initial = Tab::new(home, false);
        let id = initial.id.clone();
        tabs.tabs.push(initial);
        tabs.active_tab_id = Some(id);

        let mut engines = navigation::builtin_engines();
        for e in custom_engines {
            // Custom engine with a duplicate keyword shadows the builtin.
            engines.retain(|b| !(b.keyword == e.keyword && !e.keyword.is_empty()));
            engines.push(e);
        }
        if engines.is_empty() {
            engines = navigation::builtin_engines();
        }

        Arc::new(Self {
            db,
            tabs: Mutex::new(tabs),
            settings: Mutex::new(SettingsWithEngines { settings, engines }),
        })
    }

    /// Run `f` over the tab state, then push the new snapshot to the UI.
    pub fn with_tabs<R>(
        &self,
        app: &AppHandle,
        f: impl FnOnce(&mut TabState) -> R,
    ) -> R {
        let ret;
        let payload = {
            let mut tabs = self.tabs.lock();
            ret = f(&mut tabs);
            tabs.snapshot()
        };
        events::emit_tabs_changed(app, payload);
        ret
    }

    pub fn active_tab(&self) -> Option<Tab> {
        let tabs = self.tabs.lock();
        let id = tabs.active_tab_id.as_ref()?;
        tabs.tabs.iter().find(|t| t.id == *id).cloned()
    }

    /// Push a navigation event onto a tab's history stack.
    /// Call before changing `tab.url`.
    pub fn push_history(&self, tabs: &mut TabState, tab_id: &str, url: &str) {
        let entry = tabs.history.entry(tab_id.to_string()).or_default();
        if entry.back.last().map(|s| s.as_str()) != Some(url) {
            entry.back.push(url.to_string());
            // Cap the stack; drop the oldest entries.
            if entry.back.len() > 100 {
                let overflow = entry.back.len() - 100;
                entry.back.drain(0..overflow);
            }
        }
        entry.forward.clear();
        let can_go_back = !entry.back.is_empty();
        if let Some(t) = tabs.tabs.iter_mut().find(|t| t.id == tab_id) {
            t.can_go_back = can_go_back;
            t.can_go_forward = false;
        }
    }
}
