//! Real downloads via the wry/WebKitGTK download pipeline.
//!
//! `WebviewBuilder::on_download` gives us a `Requested` event with a mutable
//! destination path; we sanitize the filename and redirect it into the
//! configured downloads directory (never overwriting an existing file).

use crate::events::{self, DownloadSnapshot};
use tauri::AppHandle;
use url::Url;

fn downloads_dir(app: &AppHandle) -> std::path::PathBuf {
    if let Some(state) = app.try_state::<crate::state::BrowserState>() {
        if let Some(dir) = state.settings.lock().settings.download_dir.clone() {
            return std::path::PathBuf::from(dir);
        }
    }
    directories::ProjectDirs::from("", "", "RustBrowzer")
        .map(|d| d.download_dir().to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

/// Decide where a download lands. Rewrites `destination` to a sanitized,
/// non-clobbering path in the downloads directory.
pub fn on_requested(
    app: &AppHandle,
    _tab_id: &str,
    url: &Url,
    destination: &mut std::path::PathBuf,
) {
    let dir = downloads_dir(app);
    let _ = std::fs::create_dir_all(&dir);

    // Prefer the URL's last path segment, fall back to WebKit's suggestion.
    let raw_name = url
        .path_segments()
        .and_then(|mut segs| segs.next_back())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            destination
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "download".into());

    let clean = sanitize_filename::sanitize_with_options(
        &raw_name,
        sanitize_filename::Options {
            truncate: true,
            windows: cfg!(target_os = "windows"),
            replacement: "",
        },
    );
    let name = if clean.is_empty() { "download".to_string() } else { clean };

    // Never overwrite: append " (n)" before the extension.
    let mut target = dir.join(&name);
    if target.exists() {
        let stem = target
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let ext = target.extension().map(|s| s.to_string_lossy().into_owned());
        for n in 1..1000 {
            let candidate = match &ext {
                Some(ext) => dir.join(format!("{stem} ({n}).{ext}")),
                None => dir.join(format!("{stem} ({n})")),
            };
            if !candidate.exists() {
                target = candidate;
                break;
            }
        }
    }

    let id = uuid::Uuid::new_v4().to_string();
    let path_display = target.to_string_lossy().into_owned();
    if let Some(state) = app.try_state::<crate::state::BrowserState>() {
        state
            .db
            .upsert_download(&id, url.as_str(), &name, &path_display, "in_progress");
    }

    *destination = target;
    publish_downloads(app);
}

pub fn on_finished(app: &AppHandle, url: &Url, path: Option<&std::path::Path>, success: bool) {
    let Some(path) = path else { return };
    let path_display = path.to_string_lossy().into_owned();
    let status = if success { "completed" } else { "failed" };
    if let Some(state) = app.try_state::<crate::state::BrowserState>() {
        // Match the in-flight row by path (WebKit does not hand our id back).
        for row in state.db.list_downloads() {
            if row.path == path_display && row.status == "in_progress" {
                state.db.finish_download(&row.id, status, success);
                break;
            }
        }
    }
    let _ = url;
    publish_downloads(app);
}

/// Push the current download list to the UI (downloads are rare, so the whole
/// list is the simplest consistent view).
pub fn publish_downloads(app: &AppHandle) {
    let downloads: Vec<DownloadSnapshot> = app
        .try_state::<crate::state::BrowserState>()
        .map(|s| {
            s.db.list_downloads()
                .into_iter()
                .map(|d| DownloadSnapshot {
                    id: d.id,
                    url: d.url,
                    filename: d.filename,
                    path: d.path,
                    status: d.status,
                    total_bytes: d.total_bytes,
                    received_bytes: d.received_bytes,
                })
                .collect()
        })
        .unwrap_or_default();
    events::emit_downloads_changed(app, downloads);
}
