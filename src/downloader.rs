use crate::util::{now_iso, short_id};
use librqbit::api::TorrentIdOrHash;
use librqbit::{AddTorrent, AddTorrentOptions, AddTorrentResponse, ManagedTorrent, Session};
use reqwest::header::{ACCEPT_RANGES, CONTENT_DISPOSITION, CONTENT_LENGTH, RANGE};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tokio::fs::OpenOptions;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex as AsyncMutex;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTaskDto {
    pub id: String,
    pub kind: String, // "http" | "video" | "torrent"
    pub filename: String,
    pub url: String,
    pub save_path: String,
    pub status: String, // "queued" | "downloading" | "paused" | "seeding" | "completed" | "failed" | "canceled"
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub speed_bps: u64,
    pub upload_speed_bps: u64,
    pub eta_secs: u64,
    pub connections: u32,
    pub peers: u32,
    pub quality: String,
    pub error: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedMedia {
    pub url: String,
    pub filename: String,
    pub kind: String, // "video" | "hls" | "audio" | "platform"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStatusDto {
    pub ytdlp_installed: bool,
    pub ytdlp_path: String,
    pub torrent_ready: bool,
    pub download_dir: String,
}

struct TaskControl {
    paused: Arc<AtomicBool>,
    canceled: Arc<AtomicBool>,
    torrent_id: Option<usize>,
    torrent_handle: Option<Arc<ManagedTorrent>>,
}

pub struct DownloadManager {
    tasks: Arc<Mutex<Vec<DownloadTaskDto>>>,
    controls: Arc<Mutex<HashMap<String, TaskControl>>>,
    client: Client,
    torrent_session: Arc<AsyncMutex<Option<Arc<Session>>>>,
    bin_dir: PathBuf,
    download_dir: PathBuf,
}

impl DownloadManager {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let bin_dir = app_data_dir.join("bin");
        let _ = std::fs::create_dir_all(&bin_dir);
        let download_dir =
            dirs::download_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
            .connect_timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            tasks: Arc::new(Mutex::new(Vec::new())),
            controls: Arc::new(Mutex::new(HashMap::new())),
            client,
            torrent_session: Arc::new(AsyncMutex::new(None)),
            bin_dir,
            download_dir,
        }
    }

    pub fn list_tasks(&self) -> Vec<DownloadTaskDto> {
        self.tasks.lock().unwrap().clone()
    }

    pub fn emit_dm(app: &AppHandle) {
        if let Some(state) = app.try_state::<crate::state::AppState>() {
            let tasks = state.dm.list_tasks();
            let _ = app.emit_to("chrome", "dm-updated", tasks.clone());
            let _ = app.emit_to("panel", "dm-updated", tasks);
        }
    }

    pub fn resolve_ytdlp_path(&self) -> Option<PathBuf> {
        let bin_name = if cfg!(target_os = "windows") {
            "yt-dlp.exe"
        } else {
            "yt-dlp"
        };
        let local = self.bin_dir.join(bin_name);
        if local.exists() {
            return Some(local);
        }
        // Check system PATH
        let lookup_bin = if cfg!(target_os = "windows") {
            "where"
        } else {
            "which"
        };
        let mut cmd = std::process::Command::new(lookup_bin);
        cmd.arg("yt-dlp");
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        if let Ok(out) = cmd.output() {
            if out.status.success() {
                if let Some(first) = String::from_utf8_lossy(&out.stdout).lines().next() {
                    let p = PathBuf::from(first.trim());
                    if p.exists() {
                        return Some(p);
                    }
                }
            }
        }
        None
    }

    pub fn engine_status(&self) -> EngineStatusDto {
        let ytdlp = self.resolve_ytdlp_path();
        EngineStatusDto {
            ytdlp_installed: ytdlp.is_some(),
            ytdlp_path: ytdlp
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
            torrent_ready: true,
            download_dir: self.download_dir.to_string_lossy().to_string(),
        }
    }

    /// Downloads official `yt-dlp` binary into ViBrowse's app data `bin/` folder.
    pub async fn install_ytdlp(&self, app: AppHandle) -> Result<String, String> {
        let _ = std::fs::create_dir_all(&self.bin_dir);
        let (bin_name, url) = if cfg!(target_os = "windows") {
            (
                "yt-dlp.exe",
                "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe",
            )
        } else {
            (
                "yt-dlp",
                "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux",
            )
        };
        let dest = self.bin_dir.join(bin_name);

        let mut resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Gagal menghubungi GitHub: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("HTTP error saat unduh yt-dlp: {}", resp.status()));
        }

        let mut file = tokio::fs::File::create(&dest)
            .await
            .map_err(|e| format!("Gagal membuat {}: {}", bin_name, e))?;
        while let Some(bytes) = resp.chunk().await.map_err(|e| e.to_string())? {
            file.write_all(&bytes).await.map_err(|e| e.to_string())?;
        }
        file.flush().await.map_err(|e| e.to_string())?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(&dest) {
                let mut perms = meta.permissions();
                perms.set_mode(0o755);
                let _ = std::fs::set_permissions(&dest, perms);
            }
        }

        Self::emit_dm(&app);
        Ok(dest.to_string_lossy().to_string())
    }

    fn update_task(&self, id: &str, f: impl FnOnce(&mut DownloadTaskDto)) {
        let mut guard = self.tasks.lock().unwrap();
        if let Some(t) = guard.iter_mut().find(|x| x.id == id) {
            f(t);
        }
    }

    /// 1. IDM-Style Multi-Connection Segmented HTTP/HTTPS Downloader
    pub async fn start_http_download(
        &self,
        app: AppHandle,
        url: String,
        connections: u32,
        custom_filename: Option<String>,
    ) -> Result<String, String> {
        let id = short_id();
        let conn_count = connections.clamp(1, 16);
        let initial_name = custom_filename
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| infer_filename_from_url(&url));
        let save_path = unique_path(&self.download_dir, &initial_name);

        let task = DownloadTaskDto {
            id: id.clone(),
            kind: "http".to_string(),
            filename: save_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&initial_name)
                .to_string(),
            url: url.clone(),
            save_path: save_path.to_string_lossy().to_string(),
            status: "queued".to_string(),
            total_bytes: 0,
            downloaded_bytes: 0,
            speed_bps: 0,
            upload_speed_bps: 0,
            eta_secs: 0,
            connections: conn_count,
            peers: 0,
            quality: format!("{}-Conn IDM", conn_count),
            error: None,
            created_at: now_iso(),
        };

        let paused = Arc::new(AtomicBool::new(false));
        let canceled = Arc::new(AtomicBool::new(false));

        {
            self.tasks.lock().unwrap().insert(0, task);
            self.controls.lock().unwrap().insert(
                id.clone(),
                TaskControl {
                    paused: paused.clone(),
                    canceled: canceled.clone(),
                    torrent_id: None,
                    torrent_handle: None,
                },
            );
        }
        Self::emit_dm(&app);

        let client = self.client.clone();
        let tasks_arc = self.tasks.clone();
        let id_clone = id.clone();
        let app_clone = app.clone();

        tokio::spawn(async move {
            run_http_task(
                app_clone, client, tasks_arc, id_clone, url, save_path, conn_count, paused,
                canceled,
            )
            .await;
        });

        Ok(id)
    }

    /// 2. YouTube & 1000+ Platforms Video Downloader (`yt-dlp`)
    pub async fn start_video_download(
        &self,
        app: AppHandle,
        url: String,
        quality: String,
    ) -> Result<String, String> {
        let ytdlp_path = self.resolve_ytdlp_path().ok_or_else(|| {
            "yt-dlp belum terpasang. Klik tombol 'Install yt-dlp (1-Klik)' di Download Manager."
                .to_string()
        })?;

        let id = short_id();
        let quality_label = match quality.as_str() {
            "1080p" => "1080p MP4",
            "720p" => "720p MP4",
            "480p" => "480p MP4",
            "audio" => "Audio (M4A/MP3)",
            _ => "Best Quality MP4",
        };
        let format_spec = match quality.as_str() {
            "1080p" => "best[height<=1080][ext=mp4]/best[height<=1080]/best",
            "720p" => "best[height<=720][ext=mp4]/best[height<=720]/best",
            "480p" => "best[height<=480][ext=mp4]/best[height<=480]/best",
            "audio" => "bestaudio[ext=m4a]/bestaudio/best",
            _ => "best[ext=mp4]/best",
        };

        let task = DownloadTaskDto {
            id: id.clone(),
            kind: "video".to_string(),
            filename: infer_filename_from_url(&url),
            url: url.clone(),
            save_path: self.download_dir.to_string_lossy().to_string(),
            status: "downloading".to_string(),
            total_bytes: 0,
            downloaded_bytes: 0,
            speed_bps: 0,
            upload_speed_bps: 0,
            eta_secs: 0,
            connections: 4,
            peers: 0,
            quality: quality_label.to_string(),
            error: None,
            created_at: now_iso(),
        };

        let paused = Arc::new(AtomicBool::new(false));
        let canceled = Arc::new(AtomicBool::new(false));

        {
            self.tasks.lock().unwrap().insert(0, task);
            self.controls.lock().unwrap().insert(
                id.clone(),
                TaskControl {
                    paused: paused.clone(),
                    canceled: canceled.clone(),
                    torrent_id: None,
                    torrent_handle: None,
                },
            );
        }
        Self::emit_dm(&app);

        let tasks_arc = self.tasks.clone();
        let download_dir = self.download_dir.clone();
        let id_clone = id.clone();
        let format_str = format_spec.to_string();

        tokio::spawn(async move {
            let mut cmd = Command::new(&ytdlp_path);
            cmd.args([
                "--newline",
                "--progress",
                "--no-warnings",
                "--concurrent-fragments",
                "4",
                "-P",
                &download_dir.to_string_lossy(),
                "-o",
                "%(title).120s.%(ext)s",
                "-f",
                &format_str,
                &url,
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

            #[cfg(target_os = "windows")]
            cmd.creation_flags(CREATE_NO_WINDOW);

            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    update_task_arc(&tasks_arc, &id_clone, |t| {
                        t.status = "failed".to_string();
                        t.error = Some(format!("Gagal menjalankan yt-dlp: {}", e));
                    });
                    Self::emit_dm(&app);
                    return;
                }
            };

            let stdout = child.stdout.take();
            if let Some(out) = stdout {
                let mut reader = BufReader::new(out).lines();
                let mut last_emit = Instant::now();
                while let Ok(Some(line)) = reader.next_line().await {
                    if canceled.load(Ordering::Relaxed) {
                        let _ = child.kill().await;
                        update_task_arc(&tasks_arc, &id_clone, |t| {
                            t.status = "canceled".to_string();
                            t.speed_bps = 0;
                        });
                        Self::emit_dm(&app);
                        return;
                    }
                    let trimmed = line.trim();
                    if let Some(dest) = trimmed.strip_prefix("[download] Destination: ") {
                        let p = PathBuf::from(dest.trim());
                        let fname = p
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("video.mp4")
                            .to_string();
                        update_task_arc(&tasks_arc, &id_clone, |t| {
                            t.filename = fname;
                            t.save_path = dest.trim().to_string();
                        });
                        Self::emit_dm(&app);
                    } else if trimmed.starts_with("[download]") && trimmed.contains('%') {
                        if let Some((pct, total_b, speed_b, eta_s)) = parse_ytdlp_progress(trimmed)
                        {
                            update_task_arc(&tasks_arc, &id_clone, |t| {
                                if total_b > 0 {
                                    t.total_bytes = total_b;
                                    t.downloaded_bytes = ((total_b as f64) * pct / 100.0) as u64;
                                } else {
                                    t.total_bytes = 100;
                                    t.downloaded_bytes = pct as u64;
                                }
                                t.speed_bps = speed_b;
                                t.eta_secs = eta_s;
                            });
                            if last_emit.elapsed() >= Duration::from_millis(400) {
                                last_emit = Instant::now();
                                Self::emit_dm(&app);
                            }
                        }
                    }
                }
            }

            let status = child.wait().await;
            match status {
                Ok(s) if s.success() => {
                    update_task_arc(&tasks_arc, &id_clone, |t| {
                        t.status = "completed".to_string();
                        if t.total_bytes > 0 {
                            t.downloaded_bytes = t.total_bytes;
                        }
                        t.speed_bps = 0;
                        t.eta_secs = 0;
                    });
                }
                Ok(s) => {
                    update_task_arc(&tasks_arc, &id_clone, |t| {
                        if t.status != "canceled" {
                            t.status = "failed".to_string();
                            t.error = Some(format!("yt-dlp berhenti dengan kode {}", s));
                            t.speed_bps = 0;
                        }
                    });
                }
                Err(e) => {
                    update_task_arc(&tasks_arc, &id_clone, |t| {
                        t.status = "failed".to_string();
                        t.error = Some(e.to_string());
                        t.speed_bps = 0;
                    });
                }
            }
            Self::emit_dm(&app);
        });

        Ok(id)
    }

    /// 3. Native Rust BitTorrent / Magnet Client (`librqbit`)
    pub async fn start_torrent_download(
        &self,
        app: AppHandle,
        magnet_or_url: String,
    ) -> Result<String, String> {
        let torrent_dir = self.download_dir.join("ViBrowse Torrents");
        let _ = std::fs::create_dir_all(&torrent_dir);

        let session = {
            let mut guard = self.torrent_session.lock().await;
            if let Some(existing) = guard.as_ref() {
                existing.clone()
            } else {
                let s = Session::new(torrent_dir.clone())
                    .await
                    .map_err(|e| format!("Gagal memulai engine BitTorrent: {}", e))?;
                *guard = Some(s.clone());
                s
            }
        };

        let add_input = if magnet_or_url.starts_with("magnet:")
            || magnet_or_url.starts_with("http://")
            || magnet_or_url.starts_with("https://")
        {
            AddTorrent::from_url(magnet_or_url.clone())
        } else {
            let bytes = tokio::fs::read(&magnet_or_url)
                .await
                .map_err(|e| format!("Gagal membaca file .torrent: {}", e))?;
            AddTorrent::from_bytes(bytes)
        };

        let resp = session
            .add_torrent(
                add_input,
                Some(AddTorrentOptions {
                    overwrite: true,
                    ..Default::default()
                }),
            )
            .await
            .map_err(|e| format!("Gagal menambahkan torrent: {}", e))?;

        let (rq_id, handle) = match resp {
            AddTorrentResponse::Added(id, h) | AddTorrentResponse::AlreadyManaged(id, h) => (id, h),
            AddTorrentResponse::ListOnly(_) => {
                return Err("Torrent dibuka dalam mode list-only".to_string());
            }
        };

        let id = short_id();
        let name = handle
            .name()
            .unwrap_or_else(|| infer_filename_from_url(&magnet_or_url));

        let task = DownloadTaskDto {
            id: id.clone(),
            kind: "torrent".to_string(),
            filename: name,
            url: magnet_or_url,
            save_path: torrent_dir.to_string_lossy().to_string(),
            status: "downloading".to_string(),
            total_bytes: 0,
            downloaded_bytes: 0,
            speed_bps: 0,
            upload_speed_bps: 0,
            eta_secs: 0,
            connections: 0,
            peers: 0,
            quality: "BitTorrent P2P".to_string(),
            error: None,
            created_at: now_iso(),
        };

        let paused = Arc::new(AtomicBool::new(false));
        let canceled = Arc::new(AtomicBool::new(false));

        {
            self.tasks.lock().unwrap().insert(0, task);
            self.controls.lock().unwrap().insert(
                id.clone(),
                TaskControl {
                    paused: paused.clone(),
                    canceled: canceled.clone(),
                    torrent_id: Some(rq_id),
                    torrent_handle: Some(handle.clone()),
                },
            );
        }
        Self::emit_dm(&app);

        let tasks_arc = self.tasks.clone();
        let id_clone = id.clone();

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(1));
            loop {
                ticker.tick().await;
                if canceled.load(Ordering::Relaxed) {
                    break;
                }
                let stats = handle.stats();
                let (down_bps, up_bps, peers, eta_s) = if let Some(live) = &stats.live {
                    let down = (live.download_speed.mbps * 1024.0 * 1024.0) as u64;
                    let up = (live.upload_speed.mbps * 1024.0 * 1024.0) as u64;
                    let p = live.snapshot.peer_stats.live as u32;
                    let eta = if down > 0 && stats.total_bytes > stats.progress_bytes {
                        (stats.total_bytes - stats.progress_bytes) / down
                    } else {
                        0
                    };
                    (down, up, p, eta)
                } else {
                    (0, 0, 0, 0)
                };

                let new_name = handle.name();
                let new_status = if stats.error.is_some() {
                    "failed"
                } else if paused.load(Ordering::Relaxed) {
                    "paused"
                } else if stats.finished {
                    "completed"
                } else {
                    "downloading"
                };

                update_task_arc(&tasks_arc, &id_clone, |t| {
                    if let Some(n) = new_name {
                        t.filename = n;
                    }
                    t.total_bytes = stats.total_bytes;
                    t.downloaded_bytes = stats.progress_bytes;
                    t.speed_bps = down_bps;
                    t.upload_speed_bps = up_bps;
                    t.peers = peers;
                    t.connections = peers;
                    t.eta_secs = eta_s;
                    t.status = new_status.to_string();
                    t.error = stats.error.clone();
                });
                Self::emit_dm(&app);

                if stats.finished {
                    break;
                }
            }
        });

        Ok(id)
    }

    pub async fn pause_task(&self, app: AppHandle, id: &str) -> Result<(), String> {
        let torrent_handle = {
            let guard = self.controls.lock().unwrap();
            let ctrl = guard
                .get(id)
                .ok_or_else(|| "Task tidak ditemukan".to_string())?;
            ctrl.paused.store(true, Ordering::Relaxed);
            ctrl.torrent_handle.clone()
        };

        if let Some(handle) = torrent_handle {
            if let Some(session) = self.torrent_session.lock().await.as_ref() {
                let _ = session.pause(&handle).await;
            }
        }
        self.update_task(id, |t| {
            if t.status == "downloading" || t.status == "queued" {
                t.status = "paused".to_string();
                t.speed_bps = 0;
            }
        });
        Self::emit_dm(&app);
        Ok(())
    }

    pub async fn resume_task(&self, app: AppHandle, id: &str) -> Result<(), String> {
        let torrent_handle = {
            let guard = self.controls.lock().unwrap();
            let ctrl = guard
                .get(id)
                .ok_or_else(|| "Task tidak ditemukan".to_string())?;
            ctrl.paused.store(false, Ordering::Relaxed);
            ctrl.torrent_handle.clone()
        };

        if let Some(handle) = torrent_handle {
            if let Some(session) = self.torrent_session.lock().await.as_ref() {
                let _ = session.unpause(&handle).await;
            }
        }
        self.update_task(id, |t| {
            if t.status == "paused" {
                t.status = "downloading".to_string();
            }
        });
        Self::emit_dm(&app);
        Ok(())
    }

    pub async fn cancel_task(&self, app: AppHandle, id: &str) -> Result<(), String> {
        let torrent_id = {
            let guard = self.controls.lock().unwrap();
            if let Some(ctrl) = guard.get(id) {
                ctrl.canceled.store(true, Ordering::Relaxed);
                ctrl.paused.store(false, Ordering::Relaxed);
                ctrl.torrent_id
            } else {
                None
            }
        };

        if let Some(rq_id) = torrent_id {
            if let Some(session) = self.torrent_session.lock().await.as_ref() {
                let _ = session.delete(TorrentIdOrHash::Id(rq_id), false).await;
            }
        }
        self.update_task(id, |t| {
            t.status = "canceled".to_string();
            t.speed_bps = 0;
        });
        Self::emit_dm(&app);
        Ok(())
    }

    pub fn remove_task(&self, app: AppHandle, id: &str) {
        self.tasks.lock().unwrap().retain(|t| t.id != id);
        self.controls.lock().unwrap().remove(id);
        Self::emit_dm(&app);
    }

    pub fn clear_finished(&self, app: AppHandle) {
        self.tasks
            .lock()
            .unwrap()
            .retain(|t| t.status == "downloading" || t.status == "queued" || t.status == "paused");
        Self::emit_dm(&app);
    }
}

fn update_task_arc(
    tasks: &Arc<Mutex<Vec<DownloadTaskDto>>>,
    id: &str,
    f: impl FnOnce(&mut DownloadTaskDto),
) {
    let mut guard = tasks.lock().unwrap();
    if let Some(t) = guard.iter_mut().find(|x| x.id == id) {
        f(t);
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_http_task(
    app: AppHandle,
    client: Client,
    tasks: Arc<Mutex<Vec<DownloadTaskDto>>>,
    id: String,
    url: String,
    mut save_path: PathBuf,
    connections: u32,
    paused: Arc<AtomicBool>,
    canceled: Arc<AtomicBool>,
) {
    update_task_arc(&tasks, &id, |t| t.status = "downloading".to_string());
    DownloadManager::emit_dm(&app);

    // Probe server for Content-Length, Accept-Ranges, and Content-Disposition
    let mut total_bytes: u64 = 0;
    let mut supports_ranges = false;
    if let Ok(head) = client.head(&url).send().await {
        if let Some(len) = head
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
        {
            total_bytes = len;
        }
        if let Some(ar) = head
            .headers()
            .get(ACCEPT_RANGES)
            .and_then(|v| v.to_str().ok())
        {
            supports_ranges = ar.to_lowercase().contains("bytes");
        }
        if let Some(cd) = head
            .headers()
            .get(CONTENT_DISPOSITION)
            .and_then(|v| v.to_str().ok())
        {
            if let Some(fname) = extract_disposition_filename(cd) {
                if let Some(parent) = save_path.parent() {
                    save_path = unique_path(parent, &fname);
                    update_task_arc(&tasks, &id, |t| {
                        t.filename = save_path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(&fname)
                            .to_string();
                        t.save_path = save_path.to_string_lossy().to_string();
                    });
                }
            }
        }
    }

    let use_multi = supports_ranges && total_bytes >= 1_048_576 && connections > 1;
    let actual_conns = if use_multi { connections } else { 1 };

    update_task_arc(&tasks, &id, |t| {
        t.total_bytes = total_bytes;
        t.connections = actual_conns;
        t.quality = if use_multi {
            format!("{}-Conn IDM", actual_conns)
        } else {
            "Direct Stream".to_string()
        };
    });
    DownloadManager::emit_dm(&app);

    let downloaded_atomic = Arc::new(AtomicU64::new(0));

    // Speed & progress reporter task
    let stop_reporter = Arc::new(AtomicBool::new(false));
    let rep_stop = stop_reporter.clone();
    let rep_dl = downloaded_atomic.clone();
    let rep_tasks = tasks.clone();
    let rep_id = id.clone();
    let rep_app = app.clone();
    let rep_paused = paused.clone();
    let reporter = tokio::spawn(async move {
        let mut prev_bytes: u64 = 0;
        let mut ticker = tokio::time::interval(Duration::from_millis(500));
        while !rep_stop.load(Ordering::Relaxed) {
            ticker.tick().await;
            let cur = rep_dl.load(Ordering::Relaxed);
            let delta = cur.saturating_sub(prev_bytes);
            prev_bytes = cur;
            let speed = if rep_paused.load(Ordering::Relaxed) {
                0
            } else {
                delta * 2 // 500ms interval -> bytes/sec
            };
            let eta = if speed > 0 && total_bytes > cur {
                (total_bytes - cur) / speed
            } else {
                0
            };
            update_task_arc(&rep_tasks, &rep_id, |t| {
                t.downloaded_bytes = cur;
                t.speed_bps = speed;
                t.eta_secs = eta;
            });
            DownloadManager::emit_dm(&rep_app);
        }
    });

    let result: Result<(), String> = if use_multi {
        run_segmented_http(
            &client,
            &url,
            &save_path,
            total_bytes,
            actual_conns,
            &downloaded_atomic,
            &paused,
            &canceled,
        )
        .await
    } else {
        run_single_http(
            &client,
            &url,
            &save_path,
            &downloaded_atomic,
            &paused,
            &canceled,
        )
        .await
    };

    stop_reporter.store(true, Ordering::Relaxed);
    let _ = reporter.await;

    if canceled.load(Ordering::Relaxed) {
        let _ = tokio::fs::remove_file(&save_path).await;
        update_task_arc(&tasks, &id, |t| {
            t.status = "canceled".to_string();
            t.speed_bps = 0;
            t.eta_secs = 0;
        });
    } else {
        match result {
            Ok(()) => {
                let final_bytes = downloaded_atomic.load(Ordering::Relaxed);
                update_task_arc(&tasks, &id, |t| {
                    t.status = "completed".to_string();
                    t.downloaded_bytes = final_bytes;
                    if t.total_bytes == 0 {
                        t.total_bytes = final_bytes;
                    }
                    t.speed_bps = 0;
                    t.eta_secs = 0;
                });
            }
            Err(e) => {
                update_task_arc(&tasks, &id, |t| {
                    t.status = "failed".to_string();
                    t.error = Some(e);
                    t.speed_bps = 0;
                    t.eta_secs = 0;
                });
            }
        }
    }
    DownloadManager::emit_dm(&app);
}

#[allow(clippy::too_many_arguments)]
async fn run_segmented_http(
    client: &Client,
    url: &str,
    save_path: &Path,
    total_bytes: u64,
    conns: u32,
    downloaded: &Arc<AtomicU64>,
    paused: &Arc<AtomicBool>,
    canceled: &Arc<AtomicBool>,
) -> Result<(), String> {
    // Pre-allocate target file
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(save_path)
        .await
        .map_err(|e| format!("Gagal membuat file tujuan: {}", e))?;
    file.set_len(total_bytes)
        .await
        .map_err(|e| format!("Gagal mengalokasi ukuran file: {}", e))?;
    drop(file);

    let chunk_size = total_bytes / (conns as u64);
    let mut ranges = Vec::with_capacity(conns as usize);
    for i in 0..conns {
        let start = (i as u64) * chunk_size;
        let end = if i == conns - 1 {
            total_bytes - 1
        } else {
            start + chunk_size - 1
        };
        ranges.push((start, end));
    }

    let mut handles = Vec::new();
    for (idx, (start, end)) in ranges.into_iter().enumerate() {
        let c = client.clone();
        let u = url.to_string();
        let p = save_path.to_path_buf();
        let dl = downloaded.clone();
        let ps = paused.clone();
        let cn = canceled.clone();

        handles.push(tokio::spawn(async move {
            let mut f = OpenOptions::new()
                .write(true)
                .open(&p)
                .await
                .map_err(|e| e.to_string())?;
            f.seek(SeekFrom::Start(start))
                .await
                .map_err(|e| e.to_string())?;

            let mut resp = c
                .get(&u)
                .header(RANGE, format!("bytes={}-{}", start, end))
                .send()
                .await
                .map_err(|e| e.to_string())?;
            if !resp.status().is_success() {
                return Err(format!("Segment {} HTTP {}", idx, resp.status()));
            }
            while let Some(bytes) = resp.chunk().await.map_err(|e| e.to_string())? {
                if cn.load(Ordering::Relaxed) {
                    return Ok(());
                }
                while ps.load(Ordering::Relaxed) {
                    if cn.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
                f.write_all(&bytes).await.map_err(|e| e.to_string())?;
                dl.fetch_add(bytes.len() as u64, Ordering::Relaxed);
            }
            f.flush().await.map_err(|e| e.to_string())?;
            Ok::<(), String>(())
        }));
    }

    for h in handles {
        h.await.map_err(|e| e.to_string())??;
    }
    Ok(())
}

async fn run_single_http(
    client: &Client,
    url: &str,
    save_path: &Path,
    downloaded: &Arc<AtomicU64>,
    paused: &Arc<AtomicBool>,
    canceled: &Arc<AtomicBool>,
) -> Result<(), String> {
    let mut resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP error: {}", resp.status()));
    }
    let mut f = tokio::fs::File::create(save_path)
        .await
        .map_err(|e| e.to_string())?;
    while let Some(bytes) = resp.chunk().await.map_err(|e| e.to_string())? {
        if canceled.load(Ordering::Relaxed) {
            return Ok(());
        }
        while paused.load(Ordering::Relaxed) {
            if canceled.load(Ordering::Relaxed) {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        f.write_all(&bytes).await.map_err(|e| e.to_string())?;
        downloaded.fetch_add(bytes.len() as u64, Ordering::Relaxed);
    }
    f.flush().await.map_err(|e| e.to_string())?;
    Ok(())
}

fn infer_filename_from_url(raw_url: &str) -> String {
    if raw_url.starts_with("magnet:") {
        if let Ok(parsed) = url::Url::parse(raw_url) {
            for (k, v) in parsed.query_pairs() {
                if k == "dn" && !v.trim().is_empty() {
                    return sanitize_filename(&v);
                }
            }
        }
        return "Torrent Download".to_string();
    }
    if let Ok(parsed) = url::Url::parse(raw_url) {
        if let Some(seg) = parsed
            .path_segments()
            .and_then(|mut s| s.next_back())
            .filter(|s| !s.is_empty())
        {
            return sanitize_filename(seg);
        }
        if let Some(host) = parsed.host_str() {
            return sanitize_filename(host);
        }
    }
    "download.bin".to_string()
}

fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => c,
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "download.bin".to_string()
    } else {
        trimmed.chars().take(120).collect()
    }
}

fn unique_path(dir: &Path, filename: &str) -> PathBuf {
    let safe = sanitize_filename(filename);
    let candidate = dir.join(&safe);
    if !candidate.exists() {
        return candidate;
    }
    let stem = candidate
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("download");
    let ext = candidate.extension().and_then(|e| e.to_str());
    for i in 1..1000 {
        let name = match ext {
            Some(e) => format!("{} ({}).{}", stem, i, e),
            None => format!("{} ({})", stem, i),
        };
        let p = dir.join(name);
        if !p.exists() {
            return p;
        }
    }
    dir.join(format!("{}_{}", short_id(), safe))
}

fn extract_disposition_filename(header: &str) -> Option<String> {
    for part in header.split(';') {
        let p = part.trim();
        if let Some(rest) = p.strip_prefix("filename=") {
            let val = rest.trim_matches('"').trim();
            if !val.is_empty() {
                return Some(sanitize_filename(val));
            }
        }
    }
    None
}

/// Parses yt-dlp progress line: `[download]  45.2% of ~120.50MiB at   5.20MiB/s ETA 00:12`
fn parse_ytdlp_progress(line: &str) -> Option<(f64, u64, u64, u64)> {
    let pct_idx = line.find('%')?;
    let before_pct = &line[..pct_idx];
    let pct_str = before_pct.split_whitespace().next_back()?;
    let pct: f64 = pct_str.parse().ok()?;

    let mut total_bytes = 0u64;
    if let Some(of_idx) = line.find(" of ") {
        let rest = line[of_idx + 4..].trim_start_matches('~').trim();
        if let Some(tok) = rest.split_whitespace().next() {
            total_bytes = parse_human_bytes(tok);
        }
    }

    let mut speed_bps = 0u64;
    if let Some(at_idx) = line.find(" at ") {
        let rest = line[at_idx + 4..].trim();
        if let Some(tok) = rest.split_whitespace().next() {
            let clean = tok.trim_end_matches("/s");
            speed_bps = parse_human_bytes(clean);
        }
    }

    let mut eta_secs = 0u64;
    if let Some(eta_idx) = line.find(" ETA ") {
        let rest = line[eta_idx + 5..].trim();
        if let Some(tok) = rest.split_whitespace().next() {
            let parts: Vec<u64> = tok.split(':').filter_map(|p| p.parse().ok()).collect();
            eta_secs = match parts.as_slice() {
                [m, s] => m * 60 + s,
                [h, m, s] => h * 3600 + m * 60 + s,
                _ => 0,
            };
        }
    }

    Some((pct, total_bytes, speed_bps, eta_secs))
}

fn parse_human_bytes(token: &str) -> u64 {
    let lower = token.trim().to_lowercase();
    let (num_part, mult): (&str, f64) = if let Some(n) = lower.strip_suffix("gib") {
        (n, 1024.0 * 1024.0 * 1024.0)
    } else if let Some(n) = lower.strip_suffix("gb") {
        (n, 1000.0 * 1000.0 * 1000.0)
    } else if let Some(n) = lower.strip_suffix("mib") {
        (n, 1024.0 * 1024.0)
    } else if let Some(n) = lower.strip_suffix("mb") {
        (n, 1000.0 * 1000.0)
    } else if let Some(n) = lower.strip_suffix("kib") {
        (n, 1024.0)
    } else if let Some(n) = lower.strip_suffix("kb") {
        (n, 1000.0)
    } else if let Some(n) = lower.strip_suffix('b') {
        (n, 1.0)
    } else {
        return 0;
    };
    num_part
        .parse::<f64>()
        .map(|v| (v * mult) as u64)
        .unwrap_or(0)
}

/// Detects downloadable video/audio/HLS URLs for the IDM-style Media Sniffer.
pub fn sniff_media_url(raw_url: &str) -> Option<DetectedMedia> {
    let lower = raw_url.to_lowercase();
    let path_only = lower.split('?').next().unwrap_or(&lower);
    let kind = if path_only.ends_with(".m3u8") {
        "hls"
    } else if path_only.ends_with(".mp4")
        || path_only.ends_with(".webm")
        || path_only.ends_with(".mkv")
    {
        "video"
    } else if path_only.ends_with(".mp3")
        || path_only.ends_with(".m4a")
        || path_only.ends_with(".flac")
    {
        "audio"
    } else {
        return None;
    };

    Some(DetectedMedia {
        url: raw_url.to_string(),
        filename: infer_filename_from_url(raw_url),
        kind: kind.to_string(),
    })
}

/// Checks if a page URL belongs to a major video platform supported by yt-dlp.
pub fn is_video_platform_url(raw_url: &str) -> bool {
    let lower = raw_url.to_lowercase();
    [
        "youtube.com/watch",
        "youtube.com/shorts/",
        "youtu.be/",
        "tiktok.com/",
        "instagram.com/reel",
        "instagram.com/p/",
        "facebook.com/watch",
        "fb.watch/",
        "bilibili.com/video",
        "vimeo.com/",
        "twitch.tv/",
        "x.com/",
        "twitter.com/",
    ]
    .iter()
    .any(|p| lower.contains(p))
}
