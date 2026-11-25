use crate::adblock::build_tab_init_script;
use crate::downloader::{is_video_platform_url, sniff_media_url, DetectedMedia};
use crate::state::{
    is_internal_url, tab_label, AppState, ClosedTab, TabRecord, ABOUT_URL, DOWNLOADS_URL,
    NEWTAB_URL,
};
use crate::util::{extract_host, short_id};
use std::time::{Duration, Instant};
use tauri::webview::{DownloadEvent, NewWindowResponse, PageLoadEvent, WebviewBuilder};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl};
use url::Url;

pub fn emit_state(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        let snap = state.snapshot();
        let _ = app.emit_to("chrome", "browser-state", snap.clone());
        let _ = app.emit_to("panel", "browser-state", snap);
    }
}

fn internal_title(url: &str, is_incognito: bool) -> Option<String> {
    match url {
        NEWTAB_URL => Some(if is_incognito {
            "Incognito Tab".to_string()
        } else {
            "New Tab".to_string()
        }),
        DOWNLOADS_URL => Some("Download Manager".to_string()),
        ABOUT_URL => Some("About ViBrowse".to_string()),
        _ => None,
    }
}

fn is_sleep_candidate(tab: &TabRecord, active_id: Option<&str>) -> bool {
    active_id != Some(tab.id.as_str())
        && !tab.pinned
        && !tab.is_sleeping
        && tab.has_webview
        && !is_internal_url(&tab.url)
}

pub fn normalize_input_url(input: &str, search_engine: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed == NEWTAB_URL {
        return NEWTAB_URL.to_string();
    }
    if is_internal_url(trimmed)
        || trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("file://")
    {
        return trimmed.to_string();
    }
    let looks_like_host = !trimmed.contains(' ')
        && (trimmed.contains('.')
            || trimmed.starts_with("localhost")
            || trimmed.starts_with("127.0.0.1"));
    if looks_like_host {
        let candidate = if trimmed.starts_with("localhost") || trimmed.starts_with("127.0.0.1") {
            format!("http://{}", trimmed)
        } else {
            format!("https://{}", trimmed)
        };
        if Url::parse(&candidate).is_ok() {
            return candidate;
        }
    }
    let encoded: String = url::form_urlencoded::byte_serialize(trimmed.as_bytes()).collect();
    format!("{}{}", search_engine, encoded)
}

pub fn logical_window_size(app: &AppHandle) -> (f64, f64) {
    if let Some(window) = app.get_window("main") {
        let scale = window.scale_factor().unwrap_or(1.0);
        if let Ok(phys) = window.inner_size() {
            return (phys.width as f64 / scale, phys.height as f64 / scale);
        }
    }
    (1280.0, 820.0)
}

struct TabLayoutInfo {
    id: String,
    label: String,
    has_webview: bool,
    is_internal: bool,
}

pub fn set_fullscreen_mode(app: &AppHandle, fullscreen: bool) {
    if let Some(state) = app.try_state::<AppState>() {
        {
            let mut inner = state.inner.lock().unwrap();
            if inner.is_fullscreen == fullscreen {
                return;
            }
            inner.is_fullscreen = fullscreen;
        }
        if let Some(window) = app.get_window("main") {
            let _ = window.set_fullscreen(fullscreen);
        }
        layout_webviews(app);
        emit_state(app);
    }
}

pub fn toggle_fullscreen_mode(app: &AppHandle) -> bool {
    let next = app
        .try_state::<AppState>()
        .map(|s| !s.inner.lock().unwrap().is_fullscreen)
        .unwrap_or(false);
    set_fullscreen_mode(app, next);
    next
}

/// Disjoint 3-zone layout (prevents Win32 child HWND z-order occlusion).
pub fn layout_webviews(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let (win_w, win_h) = logical_window_size(app);
    let top_h = state.top_bar_height();
    let side_w = state.side_panel_width();

    let (active_id, active_is_internal, is_fullscreen, tabs_layout) = {
        let inner = state.inner.lock().unwrap();
        let active = inner.active_tab_id.clone();
        let is_int = active
            .as_deref()
            .and_then(|id| inner.tab(id))
            .map(|t| is_internal_url(&t.url))
            .unwrap_or(true);
        let list: Vec<TabLayoutInfo> = inner
            .tabs
            .iter()
            .map(|t| TabLayoutInfo {
                id: t.id.clone(),
                label: t.webview_label(),
                has_webview: t.has_webview && !t.is_sleeping,
                is_internal: is_internal_url(&t.url),
            })
            .collect();
        (active, is_int, inner.is_fullscreen, list)
    };

    let fullscreen_content = is_fullscreen && !active_is_internal;
    let content_w = (win_w - side_w).max(100.0);
    let content_h = (win_h - top_h).max(100.0);

    // 1. Chrome webview
    if let Some(chrome) = app.get_webview("chrome") {
        if fullscreen_content {
            let _ = chrome.hide();
        } else {
            let chrome_h = if active_is_internal {
                win_h.max(100.0)
            } else {
                top_h
            };
            let _ = chrome.set_position(LogicalPosition::new(0.0, 0.0));
            let _ = chrome.set_size(LogicalSize::new(win_w.max(100.0), chrome_h));
            let _ = chrome.show();
        }
    }

    // 2. Side panel webview
    if let Some(panel) = app.get_webview("panel") {
        if !fullscreen_content && !active_is_internal && side_w > 0.0 {
            let _ = panel.set_position(LogicalPosition::new(content_w, top_h));
            let _ = panel.set_size(LogicalSize::new(side_w, content_h));
            let _ = panel.show();
        } else {
            let _ = panel.hide();
        }
    }

    // 3. Content tab webviews (single unified loop)
    let (tab_y, tab_w, tab_h) = if fullscreen_content {
        (0.0, win_w.max(100.0), win_h.max(100.0))
    } else {
        (top_h, content_w, content_h)
    };

    for tab in tabs_layout {
        if !tab.has_webview {
            continue;
        }
        if let Some(wv) = app.get_webview(&tab.label) {
            let should_show = !active_is_internal
                && !tab.is_internal
                && active_id.as_deref() == Some(tab.id.as_str());
            if should_show {
                let _ = wv.set_position(LogicalPosition::new(0.0, tab_y));
                let _ = wv.set_size(LogicalSize::new(tab_w, tab_h));
                let _ = wv.show();
            } else {
                let _ = wv.hide();
            }
        }
    }
}

fn spawn_tab_webview(
    app: &AppHandle,
    tab_id: &str,
    url_str: &str,
    is_incognito: bool,
) -> Result<(), String> {
    let parsed_url = Url::parse(url_str).map_err(|e| format!("Invalid URL: {}", e))?;
    let window = app
        .get_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;
    let state = app.state::<AppState>();

    let (adblock_enabled, popup_blocker_enabled) = {
        let inner = state.inner.lock().unwrap();
        (
            inner.settings.adblock_enabled,
            inner.settings.popup_blocker_enabled,
        )
    };

    let label = tab_label(tab_id);
    if let Some(existing) = app.get_webview(&label) {
        let _ = existing.navigate(parsed_url);
        return Ok(());
    }

    let (win_w, win_h) = logical_window_size(app);
    let top_h = state.top_bar_height();
    let side_w = state.side_panel_width();
    let init_script = build_tab_init_script(
        adblock_enabled,
        popup_blocker_enabled,
        state.adblock.blocked_domains_json(),
    );

    let builder = WebviewBuilder::new(&label, WebviewUrl::External(parsed_url))
        .incognito(is_incognito)
        .initialization_script(&init_script)
        .on_navigation({
            let app = app.clone();
            let id = tab_id.to_string();
            move |nav_url| {
                let url_s = nav_url.as_str();
                if url_s.starts_with("magnet:") {
                    let magnet = url_s.to_string();
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Some(state) = app.try_state::<AppState>() {
                            let _ = state.dm.start_torrent_download(app.clone(), magnet).await;
                        }
                        let _ = create_tab(&app, Some(DOWNLOADS_URL.to_string()), false, true);
                    });
                    return false;
                }
                if let Some(state) = app.try_state::<AppState>() {
                    let adblock_enabled = state.inner.lock().unwrap().settings.adblock_enabled;
                    if state.adblock.should_block_url(url_s, adblock_enabled) {
                        emit_state(&app);
                        return false;
                    }
                    let mut hist_title: Option<String> = None;
                    {
                        let mut inner = state.inner.lock().unwrap();
                        if let Some(tab) = inner.tab_mut(&id) {
                            tab.url = url_s.to_string();
                            tab.detected_media.clear();
                            if is_video_platform_url(url_s) {
                                tab.detected_media.push(DetectedMedia {
                                    url: url_s.to_string(),
                                    filename: tab.title.clone(),
                                    kind: "platform".to_string(),
                                });
                            }
                            if !tab.is_incognito {
                                hist_title =
                                    Some(if tab.title == "New Tab" || tab.title.is_empty() {
                                        nav_url.host_str().unwrap_or(url_s).to_string()
                                    } else {
                                        tab.title.clone()
                                    });
                            }
                        }
                    }
                    if let Some(title) = hist_title {
                        state.storage.record_history(&title, url_s);
                    }
                    emit_state(&app);
                }
                true
            }
        })
        .on_document_title_changed({
            let app = app.clone();
            let id = tab_id.to_string();
            move |_wv, new_title| {
                if let Some(state) = app.try_state::<AppState>() {
                    let mut hist_pair: Option<(String, String)> = None;
                    {
                        let mut inner = state.inner.lock().unwrap();
                        if let Some(tab) = inner.tab_mut(&id) {
                            if !new_title.trim().is_empty() {
                                tab.title = new_title.clone();
                                if !tab.is_incognito {
                                    hist_pair = Some((new_title.clone(), tab.url.clone()));
                                }
                            }
                        }
                    }
                    if let Some((title, url)) = hist_pair {
                        state.storage.record_history(&title, &url);
                    }
                    emit_state(&app);
                }
            }
        })
        .on_page_load({
            let app = app.clone();
            let id = tab_id.to_string();
            move |_wv, payload| {
                if let Some(state) = app.try_state::<AppState>() {
                    {
                        let mut inner = state.inner.lock().unwrap();
                        if let Some(tab) = inner.tab_mut(&id) {
                            tab.is_loading = matches!(payload.event(), PageLoadEvent::Started);
                            tab.url = payload.url().to_string();
                        }
                    }
                    emit_state(&app);
                }
            }
        })
        .on_download({
            let app = app.clone();
            move |_wv, event| {
                if let DownloadEvent::Requested { url, destination } = event {
                    let url_s = url.to_string();
                    if url_s.starts_with("http://") || url_s.starts_with("https://") {
                        let fname = destination
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(|s| s.to_string())
                            .filter(|s| !s.is_empty());
                        let app = app.clone();
                        let is_torrent = url_s
                            .to_lowercase()
                            .split('?')
                            .next()
                            .unwrap_or("")
                            .ends_with(".torrent");
                        tauri::async_runtime::spawn(async move {
                            if let Some(state) = app.try_state::<AppState>() {
                                if is_torrent {
                                    let _ =
                                        state.dm.start_torrent_download(app.clone(), url_s).await;
                                } else {
                                    let _ = state
                                        .dm
                                        .start_http_download(app.clone(), url_s, 8, fname)
                                        .await;
                                }
                            }
                        });
                        return false;
                    }
                    let downloads_dir = dirs::download_dir()
                        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                    let filename = destination
                        .file_name()
                        .and_then(|n| n.to_str())
                        .filter(|s| !s.is_empty())
                        .unwrap_or("download.bin")
                        .to_string();
                    *destination = downloads_dir.join(filename);
                }
                true
            }
        })
        .on_new_window({
            let app = app.clone();
            let id = tab_id.to_string();
            move |new_url, _features| {
                let url_s = new_url.to_string();
                if let Some(state) = app.try_state::<AppState>() {
                    let (source_url, adblock_enabled, popup_blocker_enabled) = {
                        let inner = state.inner.lock().unwrap();
                        let src = inner.tab(&id).map(|t| t.url.clone()).unwrap_or_default();
                        (
                            src,
                            inner.settings.adblock_enabled,
                            inner.settings.popup_blocker_enabled,
                        )
                    };
                    if state.adblock.should_block_popup(
                        &source_url,
                        &url_s,
                        adblock_enabled,
                        popup_blocker_enabled,
                    ) {
                        if let Some(tab) = state.inner.lock().unwrap().tab_mut(&id) {
                            tab.last_blocked_popup = Some(url_s);
                        }
                        emit_state(&app);
                        return NewWindowResponse::Deny;
                    }
                }
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = create_tab(&app, Some(url_s), false, true);
                });
                NewWindowResponse::Deny
            }
        });

    window
        .add_child(
            builder,
            LogicalPosition::new(0.0, top_h),
            LogicalSize::new((win_w - side_w).max(100.0), (win_h - top_h).max(100.0)),
        )
        .map_err(|e| format!("Failed to create tab webview: {}", e))?;

    #[cfg(target_os = "windows")]
    attach_windows_webview_hooks(app, &label, tab_id);

    #[cfg(target_os = "linux")]
    attach_linux_webview_hooks(app, &label);

    if let Some(tab) = state.inner.lock().unwrap().tab_mut(tab_id) {
        tab.has_webview = true;
        tab.is_sleeping = false;
        tab.last_active = Instant::now();
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn attach_windows_webview_hooks(app: &AppHandle, label: &str, tab_id: &str) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_2, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
    };
    use webview2_com::{
        take_pwstr, ContainsFullScreenElementChangedEventHandler, WebResourceRequestedEventHandler,
    };
    use windows_core::{w, Interface, BOOL, PWSTR};

    let Some(wv) = app.get_webview(label) else {
        return;
    };
    let app = app.clone();
    let id = tab_id.to_string();

    let _ = wv.with_webview(move |platform_wv| unsafe {
        let Ok(core) = platform_wv.controller().CoreWebView2() else {
            return;
        };

        // 1. HTML5 Video Fullscreen -> true monitor fullscreen
        let mut fs_token: i64 = 0;
        let fs_handler = ContainsFullScreenElementChangedEventHandler::create(Box::new({
            let app = app.clone();
            move |core_opt, _| {
                if let Some(core) = core_opt {
                    let mut is_fs = BOOL::from(false);
                    if core.ContainsFullScreenElement(&mut is_fs).is_ok() {
                        set_fullscreen_mode(&app, is_fs.as_bool());
                    }
                }
                Ok(())
            }
        }));
        let _ = core.add_ContainsFullScreenElementChanged(&fs_handler, &mut fs_token);

        // 2. Native WebResourceRequested -> block ads before network & sniff media
        let _ = core.AddWebResourceRequestedFilter(w!("*"), COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL);
        let mut req_token: i64 = 0;
        let req_handler =
            WebResourceRequestedEventHandler::create(Box::new(move |core_opt, args| {
                let (Some(core), Some(args)) = (core_opt, args) else {
                    return Ok(());
                };
                let Ok(req) = args.Request() else {
                    return Ok(());
                };
                let mut uri = PWSTR::null();
                if req.Uri(&mut uri).is_err() || uri.is_null() {
                    return Ok(());
                }
                let url_str = take_pwstr(uri);
                if !url_str.starts_with("http://") && !url_str.starts_with("https://") {
                    return Ok(());
                }
                if let Some(state) = app.try_state::<AppState>() {
                    let (adblock_enabled, page_host) = {
                        let inner = state.inner.lock().unwrap();
                        let host = inner
                            .tab(&id)
                            .and_then(|t| extract_host(&t.url))
                            .unwrap_or_default();
                        (inner.settings.adblock_enabled, host)
                    };
                    if state
                        .adblock
                        .should_block_subresource(&url_str, &page_host, adblock_enabled)
                    {
                        if let Ok(wv2) = core.cast::<ICoreWebView2_2>() {
                            if let Ok(env) = wv2.Environment() {
                                if let Ok(resp) = env.CreateWebResourceResponse(
                                    None,
                                    204,
                                    w!("Blocked by ViBrowse"),
                                    w!(""),
                                ) {
                                    let _ = args.SetResponse(&resp);
                                }
                            }
                        }
                    } else if let Some(media) = sniff_media_url(&url_str) {
                        let mut added = false;
                        if let Some(tab) = state.inner.lock().unwrap().tab_mut(&id) {
                            if tab.detected_media.len() < 20
                                && !tab.detected_media.iter().any(|m| m.url == media.url)
                            {
                                tab.detected_media.push(media);
                                added = true;
                            }
                        }
                        if added {
                            emit_state(&app);
                        }
                    }
                }
                Ok(())
            }));
        let _ = core.add_WebResourceRequested(&req_handler, &mut req_token);
    });
}

#[cfg(target_os = "linux")]
fn attach_linux_webview_hooks(app: &AppHandle, label: &str) {
    use webkit2gtk::WebViewExt;

    let Some(wv) = app.get_webview(label) else {
        return;
    };
    let app = app.clone();

    let _ = wv.with_webview(move |platform_wv| {
        let gtk_wv = platform_wv.webview();
        let app_enter = app.clone();
        gtk_wv.connect_enter_fullscreen(move |_| {
            set_fullscreen_mode(&app_enter, true);
            false
        });
        gtk_wv.connect_leave_fullscreen(move |_| {
            set_fullscreen_mode(&app, false);
            false
        });
    });
}

pub fn create_tab(
    app: &AppHandle,
    url: Option<String>,
    is_incognito: bool,
    activate: bool,
) -> Result<String, String> {
    let state = app.state::<AppState>();
    let id = short_id();

    let resolved_url = {
        let inner = state.inner.lock().unwrap();
        match url {
            Some(u) if !u.trim().is_empty() => {
                normalize_input_url(&u, &inner.settings.search_engine)
            }
            _ => NEWTAB_URL.to_string(),
        }
    };

    let initial_title = internal_title(&resolved_url, is_incognito)
        .or_else(|| extract_host(&resolved_url))
        .unwrap_or_else(|| resolved_url.clone());

    let tab = TabRecord {
        id: id.clone(),
        title: initial_title,
        url: resolved_url.clone(),
        is_incognito,
        is_sleeping: false,
        is_loading: !is_internal_url(&resolved_url),
        pinned: false,
        zoom: 1.0,
        last_active: Instant::now(),
        has_webview: false,
        last_blocked_popup: None,
        detected_media: Vec::new(),
    };

    {
        let mut inner = state.inner.lock().unwrap();
        inner.tabs.push(tab);
        if activate || inner.active_tab_id.is_none() {
            inner.active_tab_id = Some(id.clone());
        }
    }

    if !is_internal_url(&resolved_url) {
        spawn_tab_webview(app, &id, &resolved_url, is_incognito)?;
    }

    layout_webviews(app);
    emit_state(app);
    Ok(id)
}

pub fn switch_tab(app: &AppHandle, tab_id: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    let wake_target = {
        let mut inner = state.inner.lock().unwrap();
        if let Some(prev_id) = inner.active_tab_id.clone() {
            if let Some(prev) = inner.tab_mut(&prev_id) {
                prev.last_active = Instant::now();
            }
        }
        let target = inner
            .tab_mut(tab_id)
            .ok_or_else(|| "Tab not found".to_string())?;
        target.last_active = Instant::now();
        let wake = if (target.is_sleeping || !target.has_webview) && !is_internal_url(&target.url) {
            Some((target.url.clone(), target.is_incognito))
        } else {
            None
        };
        inner.active_tab_id = Some(tab_id.to_string());
        wake
    };

    if let Some((url, is_incognito)) = wake_target {
        spawn_tab_webview(app, tab_id, &url, is_incognito)?;
    }

    layout_webviews(app);
    emit_state(app);
    Ok(())
}

pub fn navigate_tab(app: &AppHandle, tab_id: &str, raw_input: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (target_url, is_incognito, has_wv, label) = {
        let mut inner = state.inner.lock().unwrap();
        let normalized = normalize_input_url(raw_input, &inner.settings.search_engine);
        let tab = inner
            .tab_mut(tab_id)
            .ok_or_else(|| "Tab not found".to_string())?;
        tab.url = normalized.clone();
        tab.last_active = Instant::now();
        tab.last_blocked_popup = None;
        tab.detected_media.clear();
        if let Some(title) = internal_title(&normalized, tab.is_incognito) {
            tab.title = title;
            tab.is_loading = false;
        } else {
            tab.is_loading = true;
        }
        (
            normalized,
            tab.is_incognito,
            tab.has_webview,
            tab.webview_label(),
        )
    };

    if is_internal_url(&target_url) {
        if has_wv {
            if let Some(wv) = app.get_webview(&label) {
                let _ = wv.close();
            }
            if let Some(tab) = state.inner.lock().unwrap().tab_mut(tab_id) {
                tab.has_webview = false;
                tab.is_sleeping = false;
            }
        }
    } else if has_wv {
        if let Some(wv) = app.get_webview(&label) {
            let parsed = Url::parse(&target_url).map_err(|e| e.to_string())?;
            wv.navigate(parsed).map_err(|e| e.to_string())?;
        } else {
            spawn_tab_webview(app, tab_id, &target_url, is_incognito)?;
        }
    } else {
        spawn_tab_webview(app, tab_id, &target_url, is_incognito)?;
    }

    layout_webviews(app);
    emit_state(app);
    Ok(())
}

pub fn close_tab(app: &AppHandle, tab_id: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (label_to_close, next_active, need_new_tab) = {
        let mut inner = state.inner.lock().unwrap();
        let idx = inner
            .tabs
            .iter()
            .position(|t| t.id == tab_id)
            .ok_or_else(|| "Tab not found".to_string())?;
        let removed = inner.tabs.remove(idx);
        if !removed.is_incognito && !is_internal_url(&removed.url) {
            inner.closed_tabs.push(ClosedTab {
                url: removed.url.clone(),
                is_incognito: false,
            });
            if inner.closed_tabs.len() > 20 {
                inner.closed_tabs.remove(0);
            }
        }
        let label = removed.has_webview.then(|| removed.webview_label());

        let next = if inner.active_tab_id.as_deref() == Some(tab_id) {
            if inner.tabs.is_empty() {
                inner.active_tab_id = None;
                None
            } else {
                let next_id = inner.tabs[idx.min(inner.tabs.len() - 1)].id.clone();
                inner.active_tab_id = Some(next_id.clone());
                Some(next_id)
            }
        } else {
            None
        };

        (label, next, inner.tabs.is_empty())
    };

    if let Some(label) = label_to_close {
        if let Some(wv) = app.get_webview(&label) {
            let _ = wv.close();
        }
    }

    if need_new_tab {
        let _ = create_tab(app, None, false, true);
    } else if let Some(next_id) = next_active {
        let _ = switch_tab(app, &next_id);
    } else {
        layout_webviews(app);
        emit_state(app);
    }

    Ok(())
}

pub fn sleep_tab(app: &AppHandle, tab_id: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    let label_to_close = {
        let mut inner = state.inner.lock().unwrap();
        if inner.active_tab_id.as_deref() == Some(tab_id) {
            return Err("Cannot sleep the currently active tab".to_string());
        }
        let tab = inner
            .tab_mut(tab_id)
            .ok_or_else(|| "Tab not found".to_string())?;
        if is_internal_url(&tab.url) || !tab.has_webview || tab.is_sleeping {
            return Ok(());
        }
        tab.is_sleeping = true;
        tab.has_webview = false;
        tab.is_loading = false;
        Some(tab.webview_label())
    };

    if let Some(label) = label_to_close {
        if let Some(wv) = app.get_webview(&label) {
            let _ = wv.close();
        }
    }
    emit_state(app);
    Ok(())
}

pub fn sleep_all_inactive_tabs(app: &AppHandle) -> usize {
    let state = app.state::<AppState>();
    let targets: Vec<String> = {
        let inner = state.inner.lock().unwrap();
        let active = inner.active_tab_id.as_deref();
        inner
            .tabs
            .iter()
            .filter(|t| is_sleep_candidate(t, active))
            .map(|t| t.id.clone())
            .collect()
    };

    let count = targets.len();
    for id in targets {
        let _ = sleep_tab(app, &id);
    }
    count
}

/// Background loop that automatically puts idle tabs to sleep to save RAM.
pub fn start_tab_sleeper_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(15));
        loop {
            ticker.tick().await;
            let Some(state) = app.try_state::<AppState>() else {
                continue;
            };
            let to_sleep: Vec<String> = {
                let inner = state.inner.lock().unwrap();
                let mins = inner.settings.tab_sleep_minutes;
                if mins == 0 {
                    vec![]
                } else {
                    let threshold = Duration::from_secs(mins * 60);
                    let active = inner.active_tab_id.as_deref();
                    inner
                        .tabs
                        .iter()
                        .filter(|t| {
                            is_sleep_candidate(t, active) && t.last_active.elapsed() >= threshold
                        })
                        .map(|t| t.id.clone())
                        .collect()
                }
            };

            for id in to_sleep {
                let _ = sleep_tab(&app, &id);
            }
        }
    });
}
