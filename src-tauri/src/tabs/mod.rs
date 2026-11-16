use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::fmt;

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

impl fmt::Display for Tab {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.title)
    }
}

impl Tab {
    pub fn new(url: String, is_private: bool) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            title: "New Tab".into(),
            url,
            loading: false,
            pinned: false,
            muted: false,
            is_private,
            can_go_back: false,
            can_go_forward: false,
        }
    }

    pub fn with_title(mut self, title: String) -> Self {
        self.title = title;
        self
    }

    pub fn with_loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    pub fn toggle_pinned(&mut self) {
        self.pinned = !self.pinned;
    }

    pub fn toggle_muted(&mut self) {
        self.muted = !self.muted;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TabState {
    pub tabs: Vec<Tab>,
    pub active_tab_id: Option<String>,
    pub closed_tabs: Vec<Tab>,
}
