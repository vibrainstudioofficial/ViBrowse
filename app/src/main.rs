#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod adblock;
mod commands;
mod default_browser;
mod downloader;
mod state;
mod storage;
mod tabs;
mod updater;
mod util;

use state::AppState;
use storage::Storage;
use tabs::{create_tab, layout_webviews, start_tab_sleeper_loop};
use tauri::webview::WebviewBuilder;
use tauri::window::WindowBuilder;
use tauri::{LogicalPosition, LogicalSize, Manager, WebviewUrl, WindowEvent};

#[cfg(target_os = "windows")]
fn apply_windows_doh_from_settings() {
    if let Some(roaming) = dirs::data_dir() {
        let db_path = roaming.join("com.vibrowse.browser").join("vibrowse.db");
        if db_path.exists() {
            if let Ok(storage) = Storage::open(db_path) {
                let settings = storage.load_settings();
                if let Some(doh_url) =
                    storage::resolve_doh_template(&settings.dns_mode, &settings.custom_dns_url)
                {
                    let existing =
                        std::env::var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").unwrap_or_default();
                    let doh_flags = format!(
                        "--dns-over-https-mode=secure --dns-over-https-templates=\"{}\"",
                        doh_url
                    );
                    let merged = if existing.trim().is_empty() {
                        doh_flags
                    } else {
                        format!("{} {}", existing, doh_flags)
                    };
                    std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", merged);
                }
            }
        }
    }
}

fn main() {
    #[cfg(target_os = "windows")]
    apply_windows_doh_from_settings();

    #[cfg(target_os = "linux")]
    {
        // Prevent WebKitGTK 4.1 DMABUF compositing/repaint glitches on Ubuntu 24.04 / Linux Mint 22.x
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }

    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("vibrowse"));
            let db_path = data_dir.join("vibrowse.db");
            let storage =
                Storage::open(db_path).expect("Failed to initialize ViBrowse SQLite database");
            app.manage(AppState::new(storage, data_dir));

            let init_w = 1180.0;
            let init_h = 680.0;

            let window = WindowBuilder::new(app, "main")
                .title("ViBrowse")
                .inner_size(init_w, init_h)
                .min_inner_size(640.0, 420.0)
                .resizable(true)
                .center()
                .build()?;

            #[cfg(target_os = "linux")]
            {
                use gtk::prelude::GtkWindowExt;
                if let Ok(gtk_win) = window.gtk_window() {
                    gtk_win.set_resizable(true);
                    let geom = gdk::Geometry::new(
                        640,
                        420,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0.0,
                        0.0,
                        gdk::Gravity::NorthWest,
                    );
                    gtk_win.set_geometry_hints(
                        None::<&gtk::Widget>,
                        Some(&geom),
                        gdk::WindowHints::MIN_SIZE,
                    );
                }
            }

            window.add_child(
                WebviewBuilder::new("chrome", WebviewUrl::App("index.html".into())),
                LogicalPosition::new(0.0, 0.0),
                LogicalSize::new(init_w, init_h),
            )?;

            let panel_wv = window.add_child(
                WebviewBuilder::new("panel", WebviewUrl::App("index.html?view=panel".into())),
                LogicalPosition::new(init_w - 360.0, 84.0),
                LogicalSize::new(360.0, init_h - 84.0),
            )?;
            let _ = panel_wv.hide();

            let app_resize = app.handle().clone();
            window.on_window_event(move |event| {
                if matches!(
                    event,
                    WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }
                ) {
                    layout_webviews(&app_resize);
                }
            });

            let initial_url = std::env::args().skip(1).find(|arg| {
                let lower = arg.to_lowercase();
                lower.starts_with("http://")
                    || lower.starts_with("https://")
                    || lower.ends_with(".html")
                    || lower.ends_with(".htm")
            });
            let _ = create_tab(app.handle(), initial_url, false, true);
            start_tab_sleeper_loop(app.handle().clone());
            tauri::async_runtime::spawn_blocking(default_browser::register_browser);

            let app_ab = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                if let Some(state) = app_ab.try_state::<AppState>() {
                    if state.adblock.needs_auto_sync() {
                        let _ = state.adblock.sync_filters().await;
                        tabs::emit_state(&app_ab);
                    }
                }
            });

            let app_upd = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(4)).await;
                if let Ok(info) = updater::check_for_update().await {
                    if let Some(state) = app_upd.try_state::<AppState>() {
                        let has_upd = info.has_update;
                        {
                            let mut inner = state.inner.lock().unwrap();
                            inner.available_update = Some(info);
                            if has_upd {
                                inner.show_update_banner = true;
                            }
                        }
                        if has_upd {
                            layout_webviews(&app_upd);
                        }
                        tabs::emit_state(&app_upd);
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::cmd_new_tab,
            commands::cmd_switch_tab,
            commands::cmd_close_tab,
            commands::cmd_reopen_closed_tab,
            commands::cmd_navigate,
            commands::cmd_tab_back,
            commands::cmd_tab_forward,
            commands::cmd_tab_reload,
            commands::cmd_tab_zoom,
            commands::cmd_toggle_pin_tab,
            commands::cmd_sleep_tab,
            commands::cmd_sleep_inactive_tabs,
            commands::cmd_open_devtools,
            commands::cmd_set_panel,
            commands::cmd_list_bookmarks,
            commands::cmd_toggle_bookmark,
            commands::cmd_remove_bookmark,
            commands::cmd_detect_browser_bookmarks,
            commands::cmd_import_browser_bookmarks,
            commands::cmd_import_bookmarks_html,
            commands::cmd_export_bookmarks_html,
            commands::cmd_restart_app,
            commands::cmd_list_history,
            commands::cmd_delete_history_entry,
            commands::cmd_clear_history,
            commands::cmd_search_omnibox,
            commands::cmd_open_path,
            commands::cmd_update_settings,
            commands::cmd_toggle_adblock_site,
            commands::cmd_is_site_allowlisted,
            commands::cmd_adblock_sync_filters,
            commands::cmd_toggle_popup_site,
            commands::cmd_is_popup_site_allowlisted,
            commands::cmd_open_blocked_popup,
            commands::cmd_toggle_fullscreen,
            commands::cmd_dm_list,
            commands::cmd_dm_engine_status,
            commands::cmd_dm_install_ytdlp,
            commands::cmd_dm_start_http,
            commands::cmd_dm_start_video,
            commands::cmd_dm_start_torrent,
            commands::cmd_dm_pause,
            commands::cmd_dm_resume,
            commands::cmd_dm_cancel,
            commands::cmd_dm_remove,
            commands::cmd_dm_clear_finished,
            commands::cmd_set_default_browser,
            commands::cmd_dismiss_default_banner,
            commands::cmd_recheck_default_browser,
            commands::cmd_check_update,
            commands::cmd_install_update,
            commands::cmd_dismiss_update_banner,
            commands::cmd_clear_browsing_data,
            commands::cmd_create_profile,
            commands::cmd_switch_profile,
            commands::cmd_delete_profile,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ViBrowse");
}
