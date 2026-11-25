use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::io::AsyncWriteExt;

pub const CURRENT_VERSION: &str = "V1.2a";
const GITHUB_LATEST_URL: &str =
    "https://api.github.com/repos/fransetiaone/ViBrowse/releases/latest";
const FALLBACK_MANIFEST_URL: &str = "https://browse.vibrain.id/version.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_url: String,
    pub download_url: Option<String>,
    pub notes: String,
    pub source: String,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
    assets: Option<Vec<GhAsset>>,
}

#[derive(Deserialize)]
struct FallbackManifest {
    version: String,
    release_url: String,
    windows_setup: Option<String>,
    linux_deb: Option<String>,
    notes: Option<String>,
}

/// Parses version strings like `V1.2a`, `1.2.0-a`, `V1.3`, `2.0.1` into a comparable tuple.
/// Stable releases without a letter suffix sort higher than alpha/beta suffixes (`a`, `b`, ...).
fn parse_version_key(raw: &str) -> (u32, u32, u32, u8, String) {
    let s = raw.trim().trim_start_matches(['v', 'V']);
    let mut digits_part = String::new();
    let mut suffix = String::new();
    let mut in_suffix = false;

    for ch in s.chars() {
        if !in_suffix && (ch.is_ascii_digit() || ch == '.') {
            digits_part.push(ch);
        } else if !in_suffix && (ch == '-' || ch == '~' || ch == '_') {
            in_suffix = true;
        } else {
            in_suffix = true;
            suffix.push(ch.to_ascii_lowercase());
        }
    }

    let mut nums = digits_part
        .split('.')
        .map(|p| p.parse::<u32>().unwrap_or(0));
    let major = nums.next().unwrap_or(0);
    let minor = nums.next().unwrap_or(0);
    let patch = nums.next().unwrap_or(0);
    let stability = if suffix.is_empty() { 1 } else { 0 };
    (major, minor, patch, stability, suffix)
}

pub fn is_newer_version(latest: &str, current: &str) -> bool {
    parse_version_key(latest) > parse_version_key(current)
}

fn pick_os_asset(assets: &[GhAsset]) -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        assets
            .iter()
            .find(|a| a.name.to_lowercase().ends_with("setup.exe"))
            .or_else(|| {
                assets
                    .iter()
                    .find(|a| a.name.to_lowercase().ends_with(".exe"))
            })
            .map(|a| a.browser_download_url.clone())
    }
    #[cfg(target_os = "linux")]
    {
        assets
            .iter()
            .find(|a| a.name.to_lowercase().ends_with(".deb"))
            .or_else(|| {
                assets
                    .iter()
                    .find(|a| a.name.to_lowercase().ends_with(".rpm"))
            })
            .map(|a| a.browser_download_url.clone())
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        let _ = assets;
        None
    }
}

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("ViBrowse-Updater/1.2a")
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|e| e.to_string())
}

/// Checks GitHub Releases API first, then falls back to `https://browse.vibrain.id/version.json`.
pub async fn check_for_update() -> Result<UpdateInfo, String> {
    let client = build_client()?;

    if let Ok(resp) = client
        .get(GITHUB_LATEST_URL)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(rel) = resp.json::<GhRelease>().await {
                let assets = rel.assets.unwrap_or_default();
                return Ok(UpdateInfo {
                    current_version: CURRENT_VERSION.to_string(),
                    has_update: is_newer_version(&rel.tag_name, CURRENT_VERSION),
                    latest_version: rel.tag_name,
                    release_url: rel.html_url,
                    download_url: pick_os_asset(&assets),
                    notes: rel.body.unwrap_or_default(),
                    source: "github".to_string(),
                });
            }
        }
    }

    let fb_resp = client
        .get(FALLBACK_MANIFEST_URL)
        .send()
        .await
        .map_err(|e| format!("Gagal mengecek pembaruan: {}", e))?;
    if !fb_resp.status().is_success() {
        return Err(format!(
            "Server pembaruan mengembalikan status {}",
            fb_resp.status()
        ));
    }
    let manifest = fb_resp
        .json::<FallbackManifest>()
        .await
        .map_err(|e| format!("Format manifest pembaruan tidak valid: {}", e))?;

    let download_url = if cfg!(target_os = "windows") {
        manifest.windows_setup
    } else {
        manifest.linux_deb
    };

    Ok(UpdateInfo {
        current_version: CURRENT_VERSION.to_string(),
        has_update: is_newer_version(&manifest.version, CURRENT_VERSION),
        latest_version: manifest.version,
        release_url: manifest.release_url,
        download_url,
        notes: manifest.notes.unwrap_or_default(),
        source: "fallback".to_string(),
    })
}

/// Downloads the installer to a temporary file (updating live progress) and launches it.
pub async fn download_and_launch_update(
    app: AppHandle,
    download_url: String,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .user_agent("ViBrowse-Updater/1.2a")
        .connect_timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;

    let mut resp = client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| format!("Gagal mengunduh pembaruan: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("Gagal mengunduh (HTTP {})", resp.status()));
    }

    let total = resp.content_length().unwrap_or(0);
    let filename = if cfg!(target_os = "windows") {
        "ViBrowse-Update-Setup.exe"
    } else {
        "ViBrowse-Update.deb"
    };
    let dest = if cfg!(target_os = "windows") {
        std::env::temp_dir().join(filename)
    } else {
        dirs::download_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join(filename)
    };

    let mut file = tokio::fs::File::create(&dest)
        .await
        .map_err(|e| format!("Gagal membuat file update: {}", e))?;

    let mut downloaded: u64 = 0;
    let mut last_pct: u8 = 0;
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;
        if let Some(raw_pct) = (downloaded * 100).checked_div(total) {
            let pct = raw_pct.min(100) as u8;
            if pct != last_pct && (pct - last_pct >= 2 || pct == 100) {
                last_pct = pct;
                if let Some(state) = app.try_state::<crate::state::AppState>() {
                    state.inner.lock().unwrap().update_progress = pct;
                }
                crate::tabs::emit_state(&app);
            }
        }
    }
    file.flush().await.map_err(|e| e.to_string())?;
    drop(file);

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new(&dest)
            .spawn()
            .map_err(|e| format!("Gagal menjalankan installer pembaruan: {}", e))?;
        app.exit(0);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new("xdg-open").arg(&dest).spawn();
    }

    Ok(())
}
