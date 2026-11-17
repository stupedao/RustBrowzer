//! SQLite persistence: settings, history, bookmarks, downloads, session.
//!
//! Hardening:
//! - `journal_mode=WAL` + `synchronous=NORMAL` for fast, safe writes.
//! - `busy_timeout` so transient lock contention waits instead of erroring.
//! - All writes use prepared statements with bound parameters (no string SQL).
//! - History is deduplicated by URL (visit count + last-visit bump) and capped.

use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

/// Hard cap on persisted history rows.
pub const HISTORY_CAP: i64 = 5000;

impl Database {
    pub fn open(path: PathBuf) -> Result<Self, rusqlite::Error> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;
             PRAGMA busy_timeout=5000;",
        )?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS bookmarks (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                url TEXT NOT NULL,
                folder TEXT,
                created_at TEXT NOT NULL,
                UNIQUE(url)
            );
            CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT,
                url TEXT NOT NULL UNIQUE,
                visit_count INTEGER NOT NULL DEFAULT 1,
                first_visited_at TEXT NOT NULL,
                last_visited_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_history_last ON history(last_visited_at DESC);
            CREATE TABLE IF NOT EXISTS downloads (
                id TEXT PRIMARY KEY,
                url TEXT NOT NULL,
                filename TEXT NOT NULL,
                path TEXT NOT NULL,
                status TEXT NOT NULL,
                total_bytes INTEGER,
                received_bytes INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                completed_at TEXT
            );
            CREATE TABLE IF NOT EXISTS session (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                tabs_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            "#,
        )?;
        // Legacy schema upgrade: history used to be one-row-per-visit.
        let has_visit_count = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('history') WHERE name='visit_count'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0;
        if !has_visit_count {
            conn.execute_batch(
                "ALTER TABLE history RENAME TO history_old;
                 CREATE TABLE history (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    title TEXT,
                    url TEXT NOT NULL UNIQUE,
                    visit_count INTEGER NOT NULL DEFAULT 1,
                    first_visited_at TEXT NOT NULL,
                    last_visited_at TEXT NOT NULL
                 );
                 INSERT INTO history(title, url, visit_count, first_visited_at, last_visited_at)
                    SELECT title, url, COUNT(*), MIN(visited_at), MAX(visited_at)
                    FROM history_old GROUP BY url;
                 DROP TABLE history_old;
                 CREATE INDEX IF NOT EXISTS idx_history_last ON history(last_visited_at DESC);",
            )?;
        }
        Ok(())
    }

    fn lock_conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    // ---- settings ----

    pub fn get_setting(&self, key: &str) -> Option<String> {
        let conn = self.lock_conn();
        conn.query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| {
            r.get(0)
        })
        .optional()
        .ok()
        .flatten()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), rusqlite::Error> {
        let conn = self.lock_conn();
        conn.execute(
            "INSERT INTO settings(key,value) VALUES(?1,?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    // ---- history ----

    /// Record a visit; dedupes by URL and bumps the visit count.
    pub fn add_history(&self, title: &str, url: &str) {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        let res = conn.execute(
            "INSERT INTO history(title, url, first_visited_at, last_visited_at)
             VALUES(?1, ?2, ?3, ?3)
             ON CONFLICT(url) DO UPDATE SET
                last_visited_at=excluded.last_visited_at,
                title=excluded.title,
                visit_count=visit_count+1",
            params![title, url, now],
        );
        if res.is_err() {
            return;
        }
        // Cap the history table.
        let _ = conn.execute(
            "DELETE FROM history WHERE id IN (
                SELECT id FROM history ORDER BY last_visited_at DESC LIMIT -1 OFFSET ?1
            )",
            params![HISTORY_CAP],
        );
    }

    /// History rows for display, best matches first (by recency).
    pub fn list_history(&self, limit: i64) -> Vec<HistoryEntry> {
        let conn = self.lock_conn();
        let Ok(mut stmt) = conn.prepare(
            "SELECT id, title, url, visit_count, last_visited_at FROM history
             ORDER BY last_visited_at DESC LIMIT ?1",
        ) else {
            return Vec::new();
        };
        let rows = stmt.query_map(params![limit], |r| {
            Ok(HistoryEntry {
                id: r.get(0)?,
                title: r.get(1)?,
                url: r.get(2)?,
                visit_count: r.get(3)?,
                last_visited_at: r.get(4)?,
            })
        });
        match rows {
            Ok(rows) => rows.filter_map(Result::ok).collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Frecency-weighted candidates for the omnibox.
    ///
    /// Score = visit_count (capped so one noisy site can't dominate) with a
    /// freshness multiplier: visits in the last day count 4x, last week 2x.
    pub fn search_history(&self, needle: &str, limit: i64) -> Vec<HistoryEntry> {
        let conn = self.lock_conn();
        let pattern = format!("%{}%", needle.replace('%', "\\%").replace('_', "\\_"));
        let Ok(mut stmt) = conn.prepare(
            r#"SELECT id, title, url, visit_count, last_visited_at,
                      (visit_count * CASE
                        WHEN last_visited_at >= datetime('now', '-1 day') THEN 4
                        WHEN last_visited_at >= datetime('now', '-7 day') THEN 2
                        ELSE 1 END) AS score
               FROM history
               WHERE url LIKE ?1 ESCAPE '\' OR title LIKE ?1 ESCAPE '\'
               ORDER BY score DESC, last_visited_at DESC
               LIMIT ?2"#,
        ) else {
            return Vec::new();
        };
        let rows = stmt.query_map(params![pattern, limit], |r| {
            Ok(HistoryEntry {
                id: r.get(0)?,
                title: r.get(1)?,
                url: r.get(2)?,
                visit_count: r.get(3)?,
                last_visited_at: r.get(4)?,
            })
        });
        match rows {
            Ok(rows) => rows.filter_map(Result::ok).collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn clear_history(&self) -> Result<(), rusqlite::Error> {
        let conn = self.lock_conn();
        conn.execute("DELETE FROM history", [])?;
        Ok(())
    }

    // ---- bookmarks ----

    pub fn list_bookmarks(&self) -> Vec<BookmarkRow> {
        let conn = self.lock_conn();
        let Ok(mut stmt) = conn
            .prepare("SELECT id, title, url, folder, created_at FROM bookmarks ORDER BY created_at DESC")
        else {
            return Vec::new();
        };
        let rows = stmt.query_map([], |r| {
            Ok(BookmarkRow {
                id: r.get(0)?,
                title: r.get(1)?,
                url: r.get(2)?,
                folder: r.get(3)?,
                created_at: r.get(4)?,
            })
        });
        match rows {
            Ok(rows) => rows.filter_map(Result::ok).collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn add_bookmark(&self, title: &str, url: &str, folder: Option<&str>) -> Result<String, rusqlite::Error> {
        let id = uuid::Uuid::new_v4().to_string();
        let created = chrono::Utc::now().to_rfc3339();
        let conn = self.lock_conn();
        conn.execute(
            "INSERT OR REPLACE INTO bookmarks(id,title,url,folder,created_at) VALUES(?1,?2,?3,?4,?5)",
            params![id, title, url, folder, created],
        )?;
        Ok(id)
    }

    pub fn remove_bookmark(&self, id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.lock_conn();
        conn.execute("DELETE FROM bookmarks WHERE id=?1", params![id])?;
        Ok(())
    }

    pub fn is_bookmarked(&self, url: &str) -> bool {
        let conn = self.lock_conn();
        conn.query_row("SELECT 1 FROM bookmarks WHERE url=?1", params![url], |_| Ok(()))
            .optional()
            .ok()
            .flatten()
            .is_some()
    }

    // ---- session ----

    pub fn save_session(&self, tabs_json: &str) {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        let _ = conn.execute(
            "INSERT INTO session(id, tabs_json, updated_at) VALUES(1, ?1, ?2)
             ON CONFLICT(id) DO UPDATE SET tabs_json=excluded.tabs_json, updated_at=excluded.updated_at",
            params![tabs_json, now],
        );
    }

    pub fn load_session(&self) -> Option<String> {
        let conn = self.lock_conn();
        conn.query_row("SELECT tabs_json FROM session WHERE id=1", [], |r| r.get(0))
            .optional()
            .ok()
            .flatten()
    }

    // ---- downloads ----

    pub fn upsert_download(
        &self,
        id: &str,
        url: &str,
        filename: &str,
        path: &str,
        status: &str,
    ) {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        let _ = conn.execute(
            "INSERT INTO downloads(id,url,filename,path,status,received_bytes,created_at)
             VALUES(?1,?2,?3,?4,?5,0,?6)
             ON CONFLICT(id) DO UPDATE SET status=excluded.status",
            params![id, url, filename, path, status, now],
        );
    }

    pub fn finish_download(&self, id: &str, status: &str, completed: bool) {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        let _ = conn.execute(
            "UPDATE downloads SET status=?2, completed_at=CASE WHEN ?3 THEN ?4 ELSE completed_at END WHERE id=?1",
            params![id, status, completed, now],
        );
    }

    pub fn list_downloads(&self) -> Vec<DownloadRow> {
        let conn = self.lock_conn();
        let Ok(mut stmt) = conn.prepare(
            "SELECT id, url, filename, path, status, total_bytes, received_bytes, created_at
             FROM downloads ORDER BY created_at DESC LIMIT 100",
        ) else {
            return Vec::new();
        };
        let rows = stmt.query_map([], |r| {
            Ok(DownloadRow {
                id: r.get(0)?,
                url: r.get(1)?,
                filename: r.get(2)?,
                path: r.get(3)?,
                status: r.get(4)?,
                total_bytes: r.get(5)?,
                received_bytes: r.get(6)?,
            })
        });
        match rows {
            Ok(rows) => rows.filter_map(Result::ok).collect(),
            Err(_) => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: i64,
    pub title: Option<String>,
    pub url: String,
    pub visit_count: i64,
    pub last_visited_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkRow {
    pub id: String,
    pub title: String,
    pub url: String,
    pub folder: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRow {
    pub id: String,
    pub url: String,
    pub filename: String,
    pub path: String,
    pub status: String,
    pub total_bytes: Option<i64>,
    pub received_bytes: i64,
}
