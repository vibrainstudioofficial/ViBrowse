use crate::downloader::{DownloadTaskDto, EngineStatusDto};
use crate::state::{is_internal_url, tab_label, AppState, BrowserSnapshot};
use crate::storage::{Bookmark, BrowserSettings, HistoryEntry, OmniboxSuggestion};
use crate::tabs::{
    close_tab, create_tab, emit_state, layout_webviews, navigate_tab, sleep_all_inactive_tabs,
    sleep_tab, switch_tab, toggle_fullscreen_mode,
};
use crate::util::extract_host;
use tauri::{AppHandle, Manager, State};

fn eval_in_tab(app: &AppHandle, tab_id: &str, js: &str) {
    if let Some(wv) = app.get_webview(&tab_label(tab_id)) {
        let _ = wv.eval(js);
    }
}

#[tauri::command]
pub fn get_state(state: State<'_, AppState>) -> BrowserSnapshot {
    state.snapshot()
}

#[tauri::command]
pub async fn cmd_new_tab(
    app: AppHandle,
    url: Option<String>,
    incognito: Option<bool>,
) -> Result<String, String> {
    create_tab(&app, url, incognito.unwrap_or(false), true)
}

#[tauri::command]
pub async fn cmd_switch_tab(app: AppHandle, tab_id: String) -> Result<(), String> {
    switch_tab(&app, &tab_id)
}

#[tauri::command]
pub async fn cmd_close_tab(app: AppHandle, tab_id: String) -> Result<(), String> {
    close_tab(&app, &tab_id)
}

#[tauri::command]
pub async fn cmd_reopen_closed_tab(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let closed = state.inner.lock().unwrap().closed_tabs.pop();
    if let Some(c) = closed {
        Ok(Some(create_tab(&app, Some(c.url), c.is_incognito, true)?))
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub async fn cmd_navigate(app: AppHandle, tab_id: String, input: String) -> Result<(), String> {
    navigate_tab(&app, &tab_id, &input)
}

#[tauri::command]
pub fn cmd_tab_back(app: AppHandle, tab_id: String) -> Result<(), String> {
    eval_in_tab(&app, &tab_id, "window.history.back()");
    Ok(())
}

#[tauri::command]
pub fn cmd_tab_forward(app: AppHandle, tab_id: String) -> Result<(), String> {
    eval_in_tab(&app, &tab_id, "window.history.forward()");
    Ok(())
}

#[tauri::command]
pub fn cmd_tab_reload(app: AppHandle, tab_id: String) -> Result<(), String> {
    eval_in_tab(&app, &tab_id, "window.location.reload()");
    Ok(())
}

#[tauri::command]
pub fn cmd_tab_zoom(
    app: AppHandle,
    state: State<'_, AppState>,
    tab_id: String,
    delta: f64,
    reset: Option<bool>,
) -> Result<f64, String> {
    let new_zoom = {
        let mut inner = state.inner.lock().unwrap();
        let tab = inner
            .tab_mut(&tab_id)
            .ok_or_else(|| "Tab not found".to_string())?;
        tab.zoom = if reset.unwrap_or(false) {
            1.0
        } else {
            (tab.zoom + delta).clamp(0.5, 2.5)
        };
        tab.zoom
    };
    if let Some(wv) = app.get_webview(&tab_label(&tab_id)) {
        let _ = wv.set_zoom(new_zoom);
    }
    emit_state(&app);
    Ok(new_zoom)
}

#[tauri::command]
pub fn cmd_toggle_pin_tab(
    app: AppHandle,
    state: State<'_, AppState>,
    tab_id: String,
) -> Result<(), String> {
    if let Some(tab) = state.inner.lock().unwrap().tab_mut(&tab_id) {
        tab.pinned = !tab.pinned;
    }
    emit_state(&app);
    Ok(())
}

#[tauri::command]
pub async fn cmd_sleep_tab(app: AppHandle, tab_id: String) -> Result<(), String> {
    sleep_tab(&app, &tab_id)
}

#[tauri::command]
pub async fn cmd_sleep_inactive_tabs(app: AppHandle) -> usize {
    sleep_all_inactive_tabs(&app)
}

#[tauri::command]
pub fn cmd_open_devtools(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let active_label = {
        let guard = state.inner.lock().unwrap();
        guard
            .active_tab_id
            .as_ref()
            .and_then(|id| guard.tabs.iter().find(|t| &t.id == id))
            .filter(|t| t.has_webview && !t.is_sleeping)
            .map(|t| t.webview_label())
    };
    let target_label = active_label.as_deref().unwrap_or("chrome");
    if let Some(wv) = app.get_webview(target_label) {
        if wv.is_devtools_open() {
            wv.close_devtools();
        } else {
            wv.open_devtools();
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn cmd_set_panel(
    app: AppHandle,
    state: State<'_, AppState>,
    panel: Option<String>,
) -> Result<(), String> {
    {
        let mut guard = state.inner.lock().unwrap();
        guard.active_panel = if guard.active_panel == panel {
            None
        } else {
            panel
        };
    }
    layout_webviews(&app);
    emit_state(&app);
    Ok(())
}

// ---- Bookmarks ----

#[tauri::command]
pub fn cmd_list_bookmarks(state: State<'_, AppState>) -> Vec<Bookmark> {
    state.storage.list_bookmarks()
}

#[tauri::command]
pub fn cmd_toggle_bookmark(
    app: AppHandle,
    state: State<'_, AppState>,
    title: String,
    url: String,
) -> Result<bool, String> {
    if is_internal_url(&url) || url.trim().is_empty() {
        return Ok(false);
    }
    let added = if state.storage.is_bookmarked(&url) {
        state.storage.remove_bookmark_by_url(&url)?;
        false
    } else {
        state.storage.add_bookmark(&title, &url)?;
        true
    };
    emit_state(&app);
    Ok(added)
}

#[tauri::command]
pub fn cmd_remove_bookmark(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
) -> Result<(), String> {
    state.storage.remove_bookmark_by_url(&url)?;
    emit_state(&app);
    Ok(())
}

#[tauri::command]
pub fn cmd_detect_browser_bookmarks() -> Vec<crate::storage::DetectedBrowserBookmarks> {
    crate::storage::detect_browser_bookmarks()
}

#[tauri::command]
pub fn cmd_import_browser_bookmarks(
    app: AppHandle,
    state: State<'_, AppState>,
    browser_id: String,
) -> Result<usize, String> {
    let count = state.storage.import_browser_bookmarks(&browser_id)?;
    emit_state(&app);
    Ok(count)
}

#[tauri::command]
pub fn cmd_import_bookmarks_html(
    app: AppHandle,
    state: State<'_, AppState>,
    html: String,
) -> Result<usize, String> {
    let count = state.storage.import_bookmarks_html(&html)?;
    emit_state(&app);
    Ok(count)
}

#[tauri::command]
pub fn cmd_export_bookmarks_html(state: State<'_, AppState>) -> String {
    state.storage.export_bookmarks_html()
}

#[tauri::command]
pub fn cmd_restart_app(app: AppHandle) {
    app.restart();
}

// ---- History & Omnibox ----

#[tauri::command]
pub fn cmd_list_history(state: State<'_, AppState>, query: Option<String>) -> Vec<HistoryEntry> {
    state.storage.list_history(&query.unwrap_or_default(), 150)
}

#[tauri::command]
pub fn cmd_delete_history_entry(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.storage.delete_history_entry(id)
}

#[tauri::command]
pub fn cmd_clear_history(state: State<'_, AppState>) -> Result<(), String> {
    state.storage.clear_history()
}

#[tauri::command]
pub fn cmd_search_omnibox(state: State<'_, AppState>, query: String) -> Vec<OmniboxSuggestion> {
    state.storage.search_omnibox(&query)
}

#[tauri::command]
pub fn cmd_open_path(path: String, reveal_in_folder: Option<bool>) -> Result<(), String> {
    let reveal = reveal_in_folder.unwrap_or(false);
    #[cfg(target_os = "windows")]
    {
        if reveal {
            std::process::Command::new("explorer")
                .args(["/select,", &path])
                .spawn()
                .map_err(|e| e.to_string())?;
        } else {
            std::process::Command::new("cmd")
                .args(["/C", "start", "", &path])
                .spawn()
                .map_err(|e| e.to_string())?;
        }
    }
    #[cfg(target_os = "linux")]
    {
        let target = if reveal {
            std::path::Path::new(&path)
                .parent()
                .and_then(|p| p.to_str())
                .unwrap_or(&path)
                .to_string()
        } else {
            path
        };
        std::process::Command::new("xdg-open")
            .arg(&target)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ---- Settings & AdBlock ----

#[tauri::command]
pub async fn cmd_update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: BrowserSettings,
) -> Result<(), String> {
    state.storage.save_settings(&settings)?;
    state.inner.lock().unwrap().settings = settings;
    layout_webviews(&app);
    emit_state(&app);
    Ok(())
}

#[tauri::command]
pub fn cmd_toggle_adblock_site(app: AppHandle, state: State<'_, AppState>, host: String) -> bool {
    let allowlisted = state.adblock.toggle_allowlist(&host);
    emit_state(&app);
    allowlisted
}

#[tauri::command]
pub fn cmd_is_site_allowlisted(state: State<'_, AppState>, host: String) -> bool {
    state.adblock.is_allowlisted(&host)
}

#[tauri::command]
pub async fn cmd_adblock_sync_filters(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::adblock::AdblockStatusDto, String> {
    emit_state(&app);
    let res = state.adblock.sync_filters().await;
    emit_state(&app);
    res
}

#[tauri::command]
pub fn cmd_toggle_popup_site(app: AppHandle, state: State<'_, AppState>, host: String) -> bool {
    let allowlisted = state.adblock.toggle_popup_allowlist(&host);
    emit_state(&app);
    allowlisted
}

#[tauri::command]
pub fn cmd_is_popup_site_allowlisted(state: State<'_, AppState>, host: String) -> bool {
    state.adblock.is_popup_allowlisted(&host)
}

#[tauri::command]
pub async fn cmd_open_blocked_popup(
    app: AppHandle,
    state: State<'_, AppState>,
    tab_id: String,
    always_allow_site: Option<bool>,
) -> Result<(), String> {
    let (popup_url, source_host) = {
        let mut inner = state.inner.lock().unwrap();
        let tab = inner
            .tab_mut(&tab_id)
            .ok_or_else(|| "Tab not found".to_string())?;
        (
            tab.last_blocked_popup.take(),
            extract_host(&tab.url).unwrap_or_default(),
        )
    };

    if always_allow_site.unwrap_or(false) && !source_host.is_empty() {
        state.adblock.allow_popup_site(&source_host);
    }

    if let Some(url) = popup_url {
        let _ = create_tab(&app, Some(url), false, true);
    } else {
        emit_state(&app);
    }
    Ok(())
}

#[tauri::command]
pub async fn cmd_toggle_fullscreen(app: AppHandle) -> Result<bool, String> {
    Ok(toggle_fullscreen_mode(&app))
}

// ---- Unified Download Manager (IDM + yt-dlp + Native BitTorrent) ----

#[tauri::command]
pub fn cmd_dm_list(state: State<'_, AppState>) -> Vec<DownloadTaskDto> {
    state.dm.list_tasks()
}

#[tauri::command]
pub fn cmd_dm_engine_status(state: State<'_, AppState>) -> EngineStatusDto {
    state.dm.engine_status()
}

#[tauri::command]
pub async fn cmd_dm_install_ytdlp(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    state.dm.install_ytdlp(app).await
}

#[tauri::command]
pub async fn cmd_dm_start_http(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
    connections: Option<u32>,
    filename: Option<String>,
) -> Result<String, String> {
    state
        .dm
        .start_http_download(app, url, connections.unwrap_or(8), filename)
        .await
}

#[tauri::command]
pub async fn cmd_dm_start_video(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
    quality: Option<String>,
) -> Result<String, String> {
    state
        .dm
        .start_video_download(app, url, quality.unwrap_or_else(|| "best".to_string()))
        .await
}

#[tauri::command]
pub async fn cmd_dm_start_torrent(
    app: AppHandle,
    state: State<'_, AppState>,
    magnet_or_url: String,
) -> Result<String, String> {
    state.dm.start_torrent_download(app, magnet_or_url).await
}

#[tauri::command]
pub async fn cmd_dm_pause(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    state.dm.pause_task(app, &id).await
}

#[tauri::command]
pub async fn cmd_dm_resume(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    state.dm.resume_task(app, &id).await
}

#[tauri::command]
pub async fn cmd_dm_cancel(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    state.dm.cancel_task(app, &id).await
}

#[tauri::command]
pub fn cmd_dm_remove(app: AppHandle, state: State<'_, AppState>, id: String) {
    state.dm.remove_task(app, &id);
}

#[tauri::command]
pub fn cmd_dm_clear_finished(app: AppHandle, state: State<'_, AppState>) {
    state.dm.clear_finished(app);
}

#[tauri::command]
pub fn cmd_set_default_browser(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    crate::default_browser::set_or_open_default_browser()?;
    let is_def = crate::default_browser::is_default_browser();
    {
        let mut inner = state.inner.lock().unwrap();
        inner.is_default_browser = is_def;
        inner.show_default_banner = false;
    }
    layout_webviews(&app);
    emit_state(&app);
    Ok(())
}

#[tauri::command]
pub fn cmd_dismiss_default_banner(
    app: AppHandle,
    state: State<'_, AppState>,
    never_ask: bool,
) -> Result<(), String> {
    let settings_to_save = {
        let mut inner = state.inner.lock().unwrap();
        inner.show_default_banner = false;
        if never_ask {
            inner.settings.check_default_browser = false;
            Some(inner.settings.clone())
        } else {
            None
        }
    };
    if let Some(s) = settings_to_save {
        state.storage.save_settings(&s)?;
    }
    layout_webviews(&app);
    emit_state(&app);
    Ok(())
}

#[tauri::command]
pub fn cmd_recheck_default_browser(app: AppHandle, state: State<'_, AppState>) -> bool {
    let is_def = crate::default_browser::is_default_browser();
    {
        let mut inner = state.inner.lock().unwrap();
        inner.is_default_browser = is_def;
        if is_def {
            inner.show_default_banner = false;
        }
    }
    layout_webviews(&app);
    emit_state(&app);
    is_def
}

#[tauri::command]
pub async fn cmd_check_update(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::updater::UpdateInfo, String> {
    let info = crate::updater::check_for_update().await?;
    {
        let mut inner = state.inner.lock().unwrap();
        inner.show_update_banner = info.has_update;
        inner.available_update = Some(info.clone());
    }
    layout_webviews(&app);
    emit_state(&app);
    Ok(info)
}

#[tauri::command]
pub async fn cmd_install_update(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let dl_url = {
        let mut inner = state.inner.lock().unwrap();
        if inner.update_downloading {
            return Ok(());
        }
        let url = inner
            .available_update
            .as_ref()
            .and_then(|u| u.download_url.clone())
            .ok_or_else(|| "URL installer pembaruan tidak tersedia.".to_string())?;
        inner.update_downloading = true;
        inner.update_progress = 0;
        url
    };
    emit_state(&app);

    let res = crate::updater::download_and_launch_update(app.clone(), dl_url).await;
    if res.is_err() {
        let mut inner = state.inner.lock().unwrap();
        inner.update_downloading = false;
        inner.update_progress = 0;
        drop(inner);
        emit_state(&app);
    }
    res
}

#[tauri::command]
pub fn cmd_dismiss_update_banner(app: AppHandle, state: State<'_, AppState>) {
    state.inner.lock().unwrap().show_update_banner = false;
    layout_webviews(&app);
    emit_state(&app);
}

#[tauri::command]
pub fn cmd_clear_browsing_data(app: AppHandle) -> Result<(), String> {
    crate::tabs::clear_browsing_data(&app)
}

#[tauri::command]
pub fn cmd_create_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    color: Option<String>,
) -> Result<crate::storage::BrowserProfile, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Nama profil tidak boleh kosong".to_string());
    }
    let profile = crate::storage::BrowserProfile {
        id: crate::util::short_id(),
        name: trimmed.to_string(),
        color: color.unwrap_or_else(|| "#10b981".to_string()),
    };
    let settings = {
        let mut inner = state.inner.lock().unwrap();
        inner.settings.profiles.push(profile.clone());
        inner.settings.clone()
    };
    state.storage.save_settings(&settings)?;
    emit_state(&app);
    Ok(profile)
}

#[tauri::command]
pub fn cmd_switch_profile(app: AppHandle, profile_id: String) -> Result<(), String> {
    crate::tabs::switch_profile(&app, &profile_id)
}

#[tauri::command]
pub fn cmd_delete_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<(), String> {
    if profile_id == "default" {
        return Err("Profil Default tidak dapat dihapus".to_string());
    }
    let was_active = state.inner.lock().unwrap().settings.active_profile_id == profile_id;
    if was_active {
        crate::tabs::switch_profile(&app, "default")?;
    }
    let settings = {
        let mut inner = state.inner.lock().unwrap();
        inner.settings.profiles.retain(|p| p.id != profile_id);
        inner.settings.clone()
    };
    state.storage.save_settings(&settings)?;
    let dir = state.app_data_dir.join("profiles").join(&profile_id);
    if dir.exists() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    emit_state(&app);
    Ok(())
}
