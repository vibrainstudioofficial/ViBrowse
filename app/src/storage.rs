use crate::util::now_iso;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub visited_at: String,
    pub visit_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmniboxSuggestion {
    pub title: String,
    pub url: String,
    pub kind: String, // "bookmark" | "history"
}

fn default_true() -> bool {
    true
}

fn default_theme() -> String {
    "dark".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserSettings {
    pub search_engine: String,
    pub tab_sleep_minutes: u64, // 0 = disabled
    pub adblock_enabled: bool,
    #[serde(default = "default_true")]
    pub popup_blocker_enabled: bool,
    pub show_bookmarks_bar: bool,
    #[serde(default = "default_theme")]
    pub theme: String, // "dark" | "light"
    #[serde(default = "default_true")]
    pub check_default_browser: bool,
}

impl Default for BrowserSettings {
    fn default() -> Self {
        Self {
            search_engine: "https://www.google.com/search?q=".to_string(),
            tab_sleep_minutes: 5,
            adblock_enabled: true,
            popup_blocker_enabled: true,
            show_bookmarks_bar: true,
            theme: "dark".to_string(),
            check_default_browser: true,
        }
    }
}

pub struct Storage {
    conn: Mutex<Connection>,
}

impl Storage {
    pub fn open(db_path: PathBuf) -> Result<Self, String> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;

            CREATE TABLE IF NOT EXISTS bookmarks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                url TEXT NOT NULL UNIQUE,
                folder TEXT NOT NULL DEFAULT 'Bookmarks Bar',
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                url TEXT NOT NULL UNIQUE,
                visited_at TEXT NOT NULL,
                visit_count INTEGER NOT NULL DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            ",
        )
        .map_err(|e| e.to_string())?;

        // Seed default bookmarks on first run
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM bookmarks", [], |r| r.get(0))
            .unwrap_or(0);
        if count == 0 {
            let now = now_iso();
            for (title, url) in [
                ("Google", "https://www.google.com"),
                ("YouTube", "https://www.youtube.com"),
                ("GitHub", "https://github.com"),
                ("Wikipedia", "https://www.wikipedia.org"),
            ] {
                let _ = conn.execute(
                    "INSERT OR IGNORE INTO bookmarks (title, url, created_at) VALUES (?1, ?2, ?3)",
                    params![title, url, now],
                );
            }
        }

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    // ---- Bookmarks ----

    pub fn list_bookmarks(&self) -> Vec<Bookmark> {
        let conn = self.conn.lock().unwrap();
        let Ok(mut stmt) =
            conn.prepare("SELECT id, title, url, created_at FROM bookmarks ORDER BY id DESC")
        else {
            return vec![];
        };
        stmt.query_map([], |row| {
            Ok(Bookmark {
                id: row.get(0)?,
                title: row.get(1)?,
                url: row.get(2)?,
                created_at: row.get(3)?,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    pub fn bookmarked_urls_set(&self) -> HashSet<String> {
        let conn = self.conn.lock().unwrap();
        let Ok(mut stmt) = conn.prepare("SELECT url FROM bookmarks") else {
            return HashSet::new();
        };
        stmt.query_map([], |row| row.get::<_, String>(0))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    pub fn add_bookmark(&self, title: &str, url: &str) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO bookmarks (title, url, created_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(url) DO UPDATE SET title = excluded.title",
            params![title, url, now_iso()],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn remove_bookmark_by_url(&self, url: &str) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM bookmarks WHERE url = ?1", params![url])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn is_bookmarked(&self, url: &str) -> bool {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM bookmarks WHERE url = ?1)",
            params![url],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
            == 1
    }

    // ---- History ----

    pub fn record_history(&self, title: &str, url: &str) {
        if url.starts_with("vibrowse://") || url.is_empty() || url == "about:blank" {
            return;
        }
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "INSERT INTO history (title, url, visited_at, visit_count) VALUES (?1, ?2, ?3, 1)
             ON CONFLICT(url) DO UPDATE SET
               title = CASE WHEN length(excluded.title) > 0 THEN excluded.title ELSE history.title END,
               visited_at = excluded.visited_at,
               visit_count = history.visit_count + 1",
            params![title, url, now_iso()],
        );
    }

    pub fn list_history(&self, query: &str, limit: i64) -> Vec<HistoryEntry> {
        let conn = self.conn.lock().unwrap();
        let pattern = format!("%{}%", query.trim());
        let Ok(mut stmt) = conn.prepare(
            "SELECT id, title, url, visited_at, visit_count FROM history
             WHERE title LIKE ?1 OR url LIKE ?1
             ORDER BY visited_at DESC LIMIT ?2",
        ) else {
            return vec![];
        };
        stmt.query_map(params![pattern, limit], |row| {
            Ok(HistoryEntry {
                id: row.get(0)?,
                title: row.get(1)?,
                url: row.get(2)?,
                visited_at: row.get(3)?,
                visit_count: row.get(4)?,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    pub fn delete_history_entry(&self, id: i64) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM history WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn clear_history(&self) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM history", [])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn search_omnibox(&self, query: &str) -> Vec<OmniboxSuggestion> {
        let q = query.trim();
        if q.is_empty() {
            return vec![];
        }
        let pattern = format!("%{}%", q);
        let conn = self.conn.lock().unwrap();
        let mut results = Vec::new();

        if let Ok(mut stmt) = conn
            .prepare("SELECT title, url FROM bookmarks WHERE title LIKE ?1 OR url LIKE ?1 LIMIT 4")
        {
            if let Ok(rows) = stmt.query_map(params![&pattern], |r| {
                Ok(OmniboxSuggestion {
                    title: r.get(0)?,
                    url: r.get(1)?,
                    kind: "bookmark".to_string(),
                })
            }) {
                results.extend(rows.filter_map(|r| r.ok()));
            }
        }

        if let Ok(mut stmt) = conn.prepare(
            "SELECT title, url FROM history WHERE title LIKE ?1 OR url LIKE ?1
             ORDER BY visit_count DESC, visited_at DESC LIMIT 6",
        ) {
            if let Ok(rows) = stmt.query_map(params![&pattern], |r| {
                Ok(OmniboxSuggestion {
                    title: r.get(0)?,
                    url: r.get(1)?,
                    kind: "history".to_string(),
                })
            }) {
                for item in rows.filter_map(|r| r.ok()) {
                    if !results.iter().any(|existing| existing.url == item.url) {
                        results.push(item);
                    }
                }
            }
        }

        results.truncate(8);
        results
    }

    // ---- Settings ----

    pub fn load_settings(&self) -> BrowserSettings {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT value FROM settings WHERE key = 'browser_settings'",
            [],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
    }

    pub fn save_settings(&self, settings: &BrowserSettings) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        let json = serde_json::to_string(settings).map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('browser_settings', ?1)",
            params![json],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}
