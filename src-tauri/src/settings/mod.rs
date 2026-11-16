use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserSettings {
    pub theme: String,
    pub search_engine: String,
    pub homepage: String,
    pub download_dir: Option<String>,
    pub startup_behavior: String,
    pub show_bookmarks_bar: bool,
    pub javascript_enabled: bool,
    pub block_popups: bool,
    pub tracking_protection: bool,
}

impl Default for BrowserSettings {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            search_engine: "https://www.google.com/search?q=".into(),
            homepage: "https://example.com".into(),
            download_dir: None,
            startup_behavior: "homepage".into(),
            show_bookmarks_bar: true,
            javascript_enabled: true,
            block_popups: true,
            tracking_protection: true,
        }
    }
}
