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
pub struct BrowserProfile {
    pub id: String,
    pub name: String,
    pub color: String,
}

fn default_profiles() -> Vec<BrowserProfile> {
    vec![BrowserProfile {
        id: "default".to_string(),
        name: "Default".to_string(),
        color: "#3b82f6".to_string(),
    }]
}

fn default_profile_id() -> String {
    "default".to_string()
}

fn default_dns_mode() -> String {
    "system".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedBrowserBookmarks {
    pub id: String,
    pub name: String,
    pub count: usize,
}

pub fn resolve_doh_template(mode: &str, custom_url: &str) -> Option<String> {
    match mode {
        "cloudflare" => Some("https://cloudflare-dns.com/dns-query".to_string()),
        "google" => Some("https://dns.google/dns-query".to_string()),
        "adguard" => Some("https://dns.adguard-dns.com/dns-query".to_string()),
        "quad9" => Some("https://dns.quad9.net/dns-query".to_string()),
        "custom" => {
            let trimmed = custom_url.trim();
            if trimmed.starts_with("https://") {
                Some(trimmed.to_string())
            } else {
                None
            }
        }
        _ => None,
    }
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
    #[serde(default = "default_profiles")]
    pub profiles: Vec<BrowserProfile>,
    #[serde(default = "default_profile_id")]
    pub active_profile_id: String,
    #[serde(default = "default_dns_mode")]
    pub dns_mode: String, // "system" | "cloudflare" | "google" | "adguard" | "quad9" | "custom"
    #[serde(default)]
    pub custom_dns_url: String,
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
            profiles: default_profiles(),
            active_profile_id: default_profile_id(),
            dns_mode: default_dns_mode(),
            custom_dns_url: String::new(),
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

    pub fn import_bookmarks_batch(&self, items: &[(String, String)]) -> Result<usize, String> {
        let conn = self.conn.lock().unwrap();
        let now = now_iso();
        let mut inserted = 0usize;
        for (title, url) in items {
            let clean_url = url.trim();
            if !clean_url.starts_with("http://") && !clean_url.starts_with("https://") {
                continue;
            }
            let clean_title = if title.trim().is_empty() {
                clean_url
            } else {
                title.trim()
            };
            if let Ok(rows) = conn.execute(
                "INSERT OR IGNORE INTO bookmarks (title, url, created_at) VALUES (?1, ?2, ?3)",
                params![clean_title, clean_url, now],
            ) {
                inserted += rows;
            }
        }
        Ok(inserted)
    }

    pub fn import_browser_bookmarks(&self, browser_id: &str) -> Result<usize, String> {
        let Some((_, path)) = chromium_bookmark_paths()
            .into_iter()
            .find(|(id, _)| *id == browser_id)
        else {
            return Err("Browser tidak ditemukan".to_string());
        };
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| format!("Gagal membaca file bookmark: {}", e))?;
        let items = parse_chromium_bookmarks_json(&raw);
        if items.is_empty() {
            return Err("Tidak ada bookmark yang ditemukan di browser tersebut".to_string());
        }
        self.import_bookmarks_batch(&items)
    }

    pub fn import_bookmarks_html(&self, html: &str) -> Result<usize, String> {
        let items = parse_netscape_bookmarks_html(html);
        if items.is_empty() {
            return Err("Tidak ada bookmark valid yang ditemukan di file HTML".to_string());
        }
        self.import_bookmarks_batch(&items)
    }

    pub fn export_bookmarks_html(&self) -> String {
        let list = self.list_bookmarks();
        let mut out = String::from(
            "<!DOCTYPE NETSCAPE-Bookmark-file-1>\n\
             <META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n\
             <TITLE>ViBrowse Bookmarks</TITLE>\n\
             <H1>ViBrowse Bookmarks</H1>\n\
             <DL><p>\n",
        );
        for bm in list {
            let esc_url = bm.url.replace('&', "&amp;").replace('"', "&quot;");
            let esc_title = bm
                .title
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            out.push_str(&format!("    <DT><A HREF=\"{}\">{}</A>\n", esc_url, esc_title));
        }
        out.push_str("</DL><p>\n");
        out
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

fn chromium_bookmark_paths() -> Vec<(&'static str, PathBuf)> {
    let mut out = Vec::new();
    #[cfg(target_os = "windows")]
    if let Some(local) = dirs::data_local_dir() {
        out.push((
            "chrome",
            local.join("Google\\Chrome\\User Data\\Default\\Bookmarks"),
        ));
        out.push((
            "edge",
            local.join("Microsoft\\Edge\\User Data\\Default\\Bookmarks"),
        ));
        out.push((
            "brave",
            local.join("BraveSoftware\\Brave-Browser\\User Data\\Default\\Bookmarks"),
        ));
        out.push((
            "vivaldi",
            local.join("Vivaldi\\User Data\\Default\\Bookmarks"),
        ));
        if let Some(roaming) = dirs::data_dir() {
            out.push(("opera", roaming.join("Opera Software\\Opera Stable\\Bookmarks")));
        }
    }
    #[cfg(target_os = "linux")]
    if let Some(cfg) = dirs::config_dir() {
        out.push(("chrome", cfg.join("google-chrome/Default/Bookmarks")));
        out.push(("chromium", cfg.join("chromium/Default/Bookmarks")));
        out.push(("edge", cfg.join("microsoft-edge/Default/Bookmarks")));
        out.push((
            "brave",
            cfg.join("BraveSoftware/Brave-Browser/Default/Bookmarks"),
        ));
        out.push(("vivaldi", cfg.join("vivaldi/Default/Bookmarks")));
        out.push(("opera", cfg.join("opera/Bookmarks")));
    }
    out
}

fn browser_display_name(id: &str) -> &'static str {
    match id {
        "chrome" => "Google Chrome",
        "chromium" => "Chromium",
        "edge" => "Microsoft Edge",
        "brave" => "Brave Browser",
        "vivaldi" => "Vivaldi",
        "opera" => "Opera",
        _ => "Browser",
    }
}

fn collect_chromium_nodes(val: &serde_json::Value, out: &mut Vec<(String, String)>) {
    if let Some(obj) = val.as_object() {
        if obj.get("type").and_then(|v| v.as_str()) == Some("url") {
            if let Some(url) = obj.get("url").and_then(|v| v.as_str()) {
                let name = obj
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(url)
                    .to_string();
                out.push((name, url.to_string()));
            }
        }
        if let Some(children) = obj.get("children").and_then(|v| v.as_array()) {
            for child in children {
                collect_chromium_nodes(child, out);
            }
        }
    }
}

fn parse_chromium_bookmarks_json(raw: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Ok(json) = serde_json::from_str::<serde_json::Value>(raw) else {
        return out;
    };
    if let Some(roots) = json.get("roots").and_then(|v| v.as_object()) {
        for (_k, node) in roots {
            collect_chromium_nodes(node, &mut out);
        }
    }
    out
}

fn decode_html_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn parse_netscape_bookmarks_html(html: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut pos = 0usize;
    while let Some(rel) = lower[pos..].find("<a ") {
        let tag_start = pos + rel;
        let Some(tag_end_rel) = lower[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + tag_end_rel;
        let tag_slice = &html[tag_start..tag_end];
        let tag_lower = &lower[tag_start..tag_end];

        let mut href = None;
        if let Some(h_idx) = tag_lower.find("href=") {
            let after = &tag_slice[h_idx + 5..];
            if let Some(rest) = after.strip_prefix('"') {
                if let Some(end_q) = rest.find('"') {
                    href = Some(decode_html_entities(&rest[..end_q]));
                }
            } else if let Some(rest) = after.strip_prefix('\'') {
                if let Some(end_q) = rest.find('\'') {
                    href = Some(decode_html_entities(&rest[..end_q]));
                }
            }
        }

        let content_start = tag_end + 1;
        let Some(close_rel) = lower[content_start..].find("</a>") else {
            pos = content_start;
            continue;
        };
        let title = decode_html_entities(html[content_start..content_start + close_rel].trim());
        if let Some(url) = href {
            if url.starts_with("http://") || url.starts_with("https://") {
                out.push((title, url));
            }
        }
        pos = content_start + close_rel + 4;
    }
    out
}

pub fn detect_browser_bookmarks() -> Vec<DetectedBrowserBookmarks> {
    let mut found = Vec::new();
    for (id, path) in chromium_bookmark_paths() {
        if let Ok(raw) = std::fs::read_to_string(&path) {
            let count = parse_chromium_bookmarks_json(&raw).len();
            if count > 0 {
                found.push(DetectedBrowserBookmarks {
                    id: id.to_string(),
                    name: browser_display_name(id).to_string(),
                    count,
                });
            }
        }
    }
    found
}
