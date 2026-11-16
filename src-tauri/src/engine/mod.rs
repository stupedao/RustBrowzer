//! Engine abstraction layer.
//!
//! v1 uses Tauri/Wry system webviews for real content rendering (WebKitGTK on Linux).
//! This module defines the seam for future adapters (CEF, Servo, etc.).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineCapabilities {
    pub name: String,
    pub real_web_rendering: bool,
    pub multi_webview: bool,
    pub extension_support: String,
    pub process_isolation: String,
}

pub fn current_capabilities() -> EngineCapabilities {
    EngineCapabilities {
        name: "Tauri/Wry system webview".into(),
        real_web_rendering: true,
        multi_webview: true,
        extension_support: "not-implemented".into(),
        process_isolation: "engine-dependent".into(),
    }
}
