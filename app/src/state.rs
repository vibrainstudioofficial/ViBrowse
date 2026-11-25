use crate::adblock::{AdBlocker, AdblockStatusDto};
use crate::downloader::{DetectedMedia, DownloadManager};
use crate::storage::{BrowserSettings, Storage};
use crate::updater::UpdateInfo;
use crate::util::extract_host;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

pub const NEWTAB_URL: &str = "vibrowse://newtab";
pub const DOWNLOADS_URL: &str = "vibrowse://downloads";
pub const ABOUT_URL: &str = "vibrowse://about";
pub const TOP_BAR_BASE_HEIGHT: f64 = 84.0;
pub const BOOKMARKS_BAR_HEIGHT: f64 = 30.0;
pub const DEFAULT_BANNER_HEIGHT: f64 = 34.0;
pub const SIDE_PANEL_WIDTH: f64 = 360.0;

pub fn is_internal_url(url: &str) -> bool {
    url == NEWTAB_URL || url == DOWNLOADS_URL || url == ABOUT_URL
}

pub fn tab_label(tab_id: &str) -> String {
    format!("tab-{}", tab_id)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabDto {
    pub id: String,
    pub title: String,
    pub url: String,
    pub favicon: String,
    pub is_incognito: bool,
    pub is_sleeping: bool,
    pub is_loading: bool,
    pub pinned: bool,
    pub zoom: f64,
    pub is_bookmarked: bool,
    pub last_blocked_popup: Option<String>,
    pub detected_media: Vec<DetectedMedia>,
}

pub struct TabRecord {
    pub id: String,
    pub title: String,
    pub url: String,
    pub is_incognito: bool,
    pub is_sleeping: bool,
    pub is_loading: bool,
    pub pinned: bool,
    pub zoom: f64,
    pub last_active: Instant,
    pub has_webview: bool,
    pub last_blocked_popup: Option<String>,
    pub detected_media: Vec<DetectedMedia>,
}

impl TabRecord {
    pub fn webview_label(&self) -> String {
        tab_label(&self.id)
    }

    pub fn to_dto(&self, is_bookmarked: bool) -> TabDto {
        let favicon = if self.url.starts_with("http://") || self.url.starts_with("https://") {
            extract_host(&self.url)
                .map(|h| format!("https://www.google.com/s2/favicons?domain={}&sz=32", h))
                .unwrap_or_default()
        } else {
            String::new()
        };
        TabDto {
            id: self.id.clone(),
            title: self.title.clone(),
            url: self.url.clone(),
            favicon,
            is_incognito: self.is_incognito,
            is_sleeping: self.is_sleeping,
            is_loading: self.is_loading,
            pinned: self.pinned,
            zoom: self.zoom,
            is_bookmarked,
            last_blocked_popup: self.last_blocked_popup.clone(),
            detected_media: self.detected_media.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClosedTab {
    pub url: String,
    pub is_incognito: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserSnapshot {
    pub tabs: Vec<TabDto>,
    pub active_tab_id: Option<String>,
    pub active_panel: Option<String>,
    pub settings: BrowserSettings,
    pub adblock_blocked_count: u64,
    pub adblock_status: AdblockStatusDto,
    pub sleeping_tabs_count: usize,
    pub active_webviews_count: usize,
    pub estimated_ram_saved_mb: usize,
    pub is_fullscreen: bool,
    pub is_default_browser: bool,
    pub show_default_banner: bool,
    pub available_update: Option<UpdateInfo>,
    pub show_update_banner: bool,
    pub update_downloading: bool,
    pub update_progress: u8,
}

pub struct InnerState {
    pub tabs: Vec<TabRecord>,
    pub active_tab_id: Option<String>,
    pub closed_tabs: Vec<ClosedTab>,
    pub active_panel: Option<String>,
    pub settings: BrowserSettings,
    pub is_fullscreen: bool,
    pub is_default_browser: bool,
    pub show_default_banner: bool,
    pub available_update: Option<UpdateInfo>,
    pub show_update_banner: bool,
    pub update_downloading: bool,
    pub update_progress: u8,
}

impl InnerState {
    pub fn tab(&self, id: &str) -> Option<&TabRecord> {
        self.tabs.iter().find(|t| t.id == id)
    }

    pub fn tab_mut(&mut self, id: &str) -> Option<&mut TabRecord> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }
}

pub struct AppState {
    pub inner: Mutex<InnerState>,
    pub storage: Storage,
    pub adblock: AdBlocker,
    pub dm: DownloadManager,
    pub app_data_dir: PathBuf,
}

impl AppState {
    pub fn new(storage: Storage, app_data_dir: PathBuf) -> Self {
        let settings = storage.load_settings();
        let is_default = crate::default_browser::is_default_browser();
        let show_banner = settings.check_default_browser && !is_default;
        Self {
            inner: Mutex::new(InnerState {
                tabs: Vec::new(),
                active_tab_id: None,
                closed_tabs: Vec::new(),
                active_panel: None,
                settings,
                is_fullscreen: false,
                is_default_browser: is_default,
                show_default_banner: show_banner,
                available_update: None,
                show_update_banner: false,
                update_downloading: false,
                update_progress: 0,
            }),
            storage,
            adblock: AdBlocker::new(app_data_dir.clone()),
            dm: DownloadManager::new(app_data_dir.clone()),
            app_data_dir,
        }
    }

    /// Returns a dedicated data directory for non-default profiles so cookies/logins are isolated.
    pub fn active_profile_data_dir(&self) -> Option<PathBuf> {
        let id = self
            .inner
            .lock()
            .unwrap()
            .settings
            .active_profile_id
            .clone();
        if id.is_empty() || id == "default" {
            None
        } else {
            let dir = self.app_data_dir.join("profiles").join(id);
            let _ = std::fs::create_dir_all(&dir);
            Some(dir)
        }
    }

    pub fn snapshot(&self) -> BrowserSnapshot {
        let bookmarked = self.storage.bookmarked_urls_set();
        let inner = self.inner.lock().unwrap();
        let tabs: Vec<TabDto> = inner
            .tabs
            .iter()
            .map(|t| t.to_dto(bookmarked.contains(&t.url)))
            .collect();
        let sleeping_tabs_count = inner.tabs.iter().filter(|t| t.is_sleeping).count();
        let active_webviews_count = inner
            .tabs
            .iter()
            .filter(|t| t.has_webview && !t.is_sleeping)
            .count();
        BrowserSnapshot {
            tabs,
            active_tab_id: inner.active_tab_id.clone(),
            active_panel: inner.active_panel.clone(),
            settings: inner.settings.clone(),
            adblock_blocked_count: self.adblock.total_blocked(),
            adblock_status: self.adblock.status(),
            sleeping_tabs_count,
            active_webviews_count,
            estimated_ram_saved_mb: sleeping_tabs_count * 75,
            is_fullscreen: inner.is_fullscreen,
            is_default_browser: inner.is_default_browser,
            show_default_banner: inner.show_default_banner,
            available_update: inner.available_update.clone(),
            show_update_banner: inner.show_update_banner,
            update_downloading: inner.update_downloading,
            update_progress: inner.update_progress,
        }
    }

    pub fn top_bar_height(&self) -> f64 {
        let inner = self.inner.lock().unwrap();
        let mut h = TOP_BAR_BASE_HEIGHT;
        if inner.settings.show_bookmarks_bar {
            h += BOOKMARKS_BAR_HEIGHT;
        }
        if inner.show_default_banner {
            h += DEFAULT_BANNER_HEIGHT;
        }
        if inner.show_update_banner {
            h += DEFAULT_BANNER_HEIGHT;
        }
        h
    }

    pub fn side_panel_width(&self) -> f64 {
        if self.inner.lock().unwrap().active_panel.is_some() {
            SIDE_PANEL_WIDTH
        } else {
            0.0
        }
    }
}
