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
use tokio::sync::{Mutex as AsyncMutex, Notify};

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Queued,
    Downloading,
    Paused,
    Completed,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTaskDto {
    pub id: String,
    pub kind: String, // "http" | "video" | "torrent"
    pub filename: String,
    pub url: String,
    pub save_path: String,
    pub status: TaskStatus,
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

impl DownloadTaskDto {
    #[allow(clippy::too_many_arguments)]
    fn new(
        id: String,
        kind: &str,
        filename: String,
        url: String,
        save_path: String,
        status: TaskStatus,
        connections: u32,
        quality: String,
    ) -> Self {
        Self {
            id,
            kind: kind.to_string(),
            filename,
            url,
            save_path,
            status,
            total_bytes: 0,
            downloaded_bytes: 0,
            speed_bps: 0,
            upload_speed_bps: 0,
            eta_secs: 0,
            connections,
            peers: 0,
            quality,
            error: None,
            created_at: now_iso(),
        }
    }
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
}

struct TaskControl {
    paused: Arc<AtomicBool>,
    canceled: Arc<AtomicBool>,
    notify: Arc<Notify>,
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

fn ytdlp_bin_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    }
}

fn update_task(
    tasks: &Arc<Mutex<Vec<DownloadTaskDto>>>,
    id: &str,
    f: impl FnOnce(&mut DownloadTaskDto),
) {
    let mut list = tasks.lock().unwrap();
    if let Some(task) = list.iter_mut().find(|t| t.id == id) {
        f(task);
    }
}

/// Blocks while `paused` is true and returns `true` if `canceled` was triggered.
async fn wait_if_paused(paused: &AtomicBool, canceled: &AtomicBool, notify: &Notify) -> bool {
    if canceled.load(Ordering::Relaxed) {
        return true;
    }
    while paused.load(Ordering::Relaxed) {
        if canceled.load(Ordering::Relaxed) {
            return true;
        }
        let _ = tokio::time::timeout(Duration::from_millis(250), notify.notified()).await;
    }
    canceled.load(Ordering::Relaxed)
}

/// Immediately terminates a child process and its process tree (e.g. `yt-dlp` + `ffmpeg`).
async fn kill_process_tree(child: &mut tokio::process::Child) {
    #[cfg(target_os = "windows")]
    if let Some(pid) = child.id() {
        let mut killer = std::process::Command::new("taskkill");
        killer.args(["/F", "/T", "/PID", &pid.to_string()]);
        use std::os::windows::process::CommandExt;
        killer.creation_flags(CREATE_NO_WINDOW);
        let _ = killer.output();
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
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

    fn register_task(
        &self,
        app: &AppHandle,
        task: DownloadTaskDto,
        torrent_id: Option<usize>,
        torrent_handle: Option<Arc<ManagedTorrent>>,
    ) -> (Arc<AtomicBool>, Arc<AtomicBool>, Arc<Notify>) {
        let paused = Arc::new(AtomicBool::new(false));
        let canceled = Arc::new(AtomicBool::new(false));
        let notify = Arc::new(Notify::new());
        let id = task.id.clone();

        self.tasks.lock().unwrap().insert(0, task);
        self.controls.lock().unwrap().insert(
            id,
            TaskControl {
                paused: paused.clone(),
                canceled: canceled.clone(),
                notify: notify.clone(),
                torrent_id,
                torrent_handle,
            },
        );
        Self::emit_dm(app);
        (paused, canceled, notify)
    }

    pub fn resolve_binary_in_path(name: &str) -> Option<PathBuf> {
        let lookup = if cfg!(target_os = "windows") {
            "where"
        } else {
            "which"
        };
        let mut cmd = std::process::Command::new(lookup);
        cmd.arg(name);
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        cmd.output()
            .ok()
            .filter(|out| out.status.success())
            .and_then(|out| {
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .next()
                    .map(|line| PathBuf::from(line.trim()))
            })
            .filter(|p| p.exists())
    }

    pub fn resolve_ytdlp_path(&self) -> Option<PathBuf> {
        let local = self.bin_dir.join(ytdlp_bin_name());
        if local.exists() {
            return Some(local);
        }
        Self::resolve_binary_in_path("yt-dlp")
    }

    pub fn engine_status(&self) -> EngineStatusDto {
        EngineStatusDto {
            ytdlp_installed: self.resolve_ytdlp_path().is_some(),
        }
    }

    /// Downloads official `yt-dlp` binary into ViBrowse's app data `bin/` folder.
    pub async fn install_ytdlp(&self, app: AppHandle) -> Result<String, String> {
        let _ = std::fs::create_dir_all(&self.bin_dir);
        let bin_name = ytdlp_bin_name();
        let url = if cfg!(target_os = "windows") {
            "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
        } else {
            "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux"
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
        let filename = save_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&initial_name)
            .to_string();

        let task = DownloadTaskDto::new(
            id.clone(),
            "http",
            filename,
            url.clone(),
            save_path.to_string_lossy().to_string(),
            TaskStatus::Queued,
            conn_count,
            format!("{}-Conn IDM", conn_count),
        );
        let (paused, canceled, notify) = self.register_task(&app, task, None, None);

        let client = self.client.clone();
        let tasks = self.tasks.clone();
        let task_id = id.clone();

        tauri::async_runtime::spawn(async move {
            run_http_task(
                app, client, tasks, task_id, url, save_path, conn_count, paused, canceled, notify,
            )
            .await;
        });

        Ok(id)
    }

    /// 2. YouTube, X/Twitter, & 1000+ Platforms Video Downloader (`yt-dlp`, auto-installs if missing)
    pub async fn start_video_download(
        &self,
        app: AppHandle,
        url: String,
        quality: String,
    ) -> Result<String, String> {
        let url = normalize_video_url(&url);
        // Direct MP4/WebM/MKV links download faster via 8-connection IDM HTTP engine.
        let clean_path = url.to_lowercase();
        let path_no_q = clean_path.split('?').next().unwrap_or("");
        if [".mp4", ".webm", ".mkv", ".mov", ".mp3", ".m4a"]
            .iter()
            .any(|ext| path_no_q.ends_with(ext))
        {
            return self.start_http_download(app, url, 8, None).await;
        }

        let id = short_id();
        let has_ffmpeg = Self::resolve_binary_in_path("ffmpeg").is_some();
        let (quality_label, format_spec) = match quality.as_str() {
            "1080p" => (
                "1080p MP4",
                if has_ffmpeg {
                    "bv*[height<=1080][ext=mp4]+ba[ext=m4a]/b[height<=1080][ext=mp4]/bv*[height<=1080]+ba/b[height<=1080]/b"
                } else {
                    "b[height<=1080][ext=mp4]/b[height<=1080]/b[ext=mp4]/b"
                },
            ),
            "720p" => (
                "720p MP4",
                if has_ffmpeg {
                    "bv*[height<=720][ext=mp4]+ba[ext=m4a]/b[height<=720][ext=mp4]/bv*[height<=720]+ba/b[height<=720]/b"
                } else {
                    "b[height<=720][ext=mp4]/b[height<=720]/b[ext=mp4]/b"
                },
            ),
            "480p" => (
                "480p MP4",
                if has_ffmpeg {
                    "bv*[height<=480][ext=mp4]+ba[ext=m4a]/b[height<=480][ext=mp4]/bv*[height<=480]+ba/b[height<=480]/b"
                } else {
                    "b[height<=480][ext=mp4]/b[height<=480]/b[ext=mp4]/b"
                },
            ),
            "audio" => ("Audio (M4A)", "ba[ext=m4a]/ba/b"),
            _ => (
                "Best Quality MP4",
                if has_ffmpeg {
                    "bv*[ext=mp4]+ba[ext=m4a]/b[ext=mp4]/bv*+ba/b"
                } else {
                    "b[ext=mp4]/b"
                },
            ),
        };

        let existing_ytdlp = self.resolve_ytdlp_path();
        let initial_name = if existing_ytdlp.is_some() {
            infer_filename_from_url(&url)
        } else {
            "Menyiapkan engine yt-dlp...".to_string()
        };

        let task = DownloadTaskDto::new(
            id.clone(),
            "video",
            initial_name,
            url.clone(),
            self.download_dir.to_string_lossy().to_string(),
            TaskStatus::Downloading,
            4,
            quality_label.to_string(),
        );
        let (paused, canceled, notify) = self.register_task(&app, task, None, None);

        let tasks = self.tasks.clone();
        let download_dir = self.download_dir.clone();
        let task_id = id.clone();
        let format_str = format_spec.to_string();

        tauri::async_runtime::spawn(async move {
            let ytdlp_path = match existing_ytdlp {
                Some(p) => p,
                None => {
                    let install_res = if let Some(state) = app.try_state::<crate::state::AppState>()
                    {
                        state.dm.install_ytdlp(app.clone()).await
                    } else {
                        Err("AppState tidak tersedia".to_string())
                    };
                    match install_res {
                        Ok(p) => {
                            update_task(&tasks, &task_id, |t| {
                                t.filename = infer_filename_from_url(&url);
                            });
                            Self::emit_dm(&app);
                            PathBuf::from(p)
                        }
                        Err(e) => {
                            update_task(&tasks, &task_id, |t| {
                                t.status = TaskStatus::Failed;
                                t.error = Some(format!("Auto-install yt-dlp gagal: {}", e));
                            });
                            Self::emit_dm(&app);
                            return;
                        }
                    }
                }
            };

            // Two-pass execution with true Pause/Resume (`--continue`) and instant Stop (`taskkill`).
            let mut last_error = String::new();
            let mut succeeded = false;
            let mut attempt = 0u8;

            while attempt < 2 {
                if wait_if_paused(&paused, &canceled, &notify).await {
                    update_task(&tasks, &task_id, |t| {
                        t.status = TaskStatus::Canceled;
                        t.speed_bps = 0;
                        t.eta_secs = 0;
                    });
                    Self::emit_dm(&app);
                    return;
                }
                let mut args: Vec<String> = vec![
                    "--continue".into(),
                    "--no-playlist".into(),
                    "--windows-filenames".into(),
                    "--no-check-certificates".into(),
                    "--newline".into(),
                    "--progress".into(),
                    "--retries".into(),
                    "10".into(),
                    "--fragment-retries".into(),
                    "10".into(),
                    "--socket-timeout".into(),
                    "30".into(),
                    "--concurrent-fragments".into(),
                    "4".into(),
                    "-P".into(),
                    download_dir.to_string_lossy().to_string(),
                    "-o".into(),
                    "%(title).120s.%(ext)s".into(),
                    "-f".into(),
                    if attempt == 0 {
                        format_str.clone()
                    } else {
                        "best/b".into()
                    },
                ];
                if attempt == 0 {
                    if Self::resolve_binary_in_path("deno").is_some() {
                        args.extend([
                            "--js-runtimes".into(),
                            "deno".into(),
                            "--remote-components".into(),
                            "ejs:github".into(),
                        ]);
                    } else if Self::resolve_binary_in_path("node").is_some() {
                        args.extend([
                            "--js-runtimes".into(),
                            "node".into(),
                            "--remote-components".into(),
                            "ejs:github".into(),
                        ]);
                    } else if Self::resolve_binary_in_path("bun").is_some() {
                        args.extend([
                            "--js-runtimes".into(),
                            "bun".into(),
                            "--remote-components".into(),
                            "ejs:github".into(),
                        ]);
                    } else {
                        args.extend([
                            "--extractor-args".into(),
                            "youtube:player_client=android_vr,ios,web".into(),
                        ]);
                    }
                } else {
                    args.extend([
                        "--extractor-args".into(),
                        "youtube:player_client=android_vr,ios,tv_simply,web".into(),
                    ]);
                }
                if has_ffmpeg && quality != "audio" && attempt == 0 {
                    args.push("--merge-output-format".into());
                    args.push("mp4".into());
                }
                args.push(url.clone());

                let mut cmd = Command::new(&ytdlp_path);
                cmd.args(&args)
                    .env("PYTHONUTF8", "1")
                    .env("PYTHONIOENCODING", "utf-8")
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped());

                #[cfg(target_os = "windows")]
                cmd.creation_flags(CREATE_NO_WINDOW);

                let mut child = match cmd.spawn() {
                    Ok(c) => c,
                    Err(e) => {
                        last_error = format!("Gagal menjalankan yt-dlp: {}", e);
                        break;
                    }
                };

                let stderr_handle = child.stderr.take().map(|err_io| {
                    tauri::async_runtime::spawn(async move {
                        let mut reader = BufReader::new(err_io);
                        let mut buf = Vec::new();
                        let mut last_err = String::new();
                        while let Ok(n) = reader.read_until(b'\n', &mut buf).await {
                            if n == 0 {
                                break;
                            }
                            let raw = String::from_utf8_lossy(&buf);
                            let t = raw.trim();
                            if !t.is_empty() && (t.starts_with("ERROR:") || last_err.is_empty()) {
                                last_err = t.to_string();
                            }
                            buf.clear();
                        }
                        last_err
                    })
                });

                let mut was_paused = false;
                if let Some(out) = child.stdout.take() {
                    let mut reader = BufReader::new(out);
                    let mut buf = Vec::new();
                    let mut last_emit = Instant::now();
                    loop {
                        buf.clear();
                        let read_res = tokio::select! {
                            biased;
                            _ = notify.notified() => {
                                if canceled.load(Ordering::Relaxed) {
                                    kill_process_tree(&mut child).await;
                                    update_task(&tasks, &task_id, |t| {
                                        t.status = TaskStatus::Canceled;
                                        t.speed_bps = 0;
                                        t.eta_secs = 0;
                                    });
                                    Self::emit_dm(&app);
                                    return;
                                }
                                if paused.load(Ordering::Relaxed) {
                                    kill_process_tree(&mut child).await;
                                    update_task(&tasks, &task_id, |t| {
                                        t.status = TaskStatus::Paused;
                                        t.speed_bps = 0;
                                        t.eta_secs = 0;
                                    });
                                    Self::emit_dm(&app);
                                    was_paused = true;
                                    break;
                                }
                                continue;
                            }
                            r = reader.read_until(b'\n', &mut buf) => r,
                        };
                        let Ok(n) = read_res else {
                            break;
                        };
                        if n == 0 {
                            break;
                        }
                        let line = String::from_utf8_lossy(&buf).into_owned();
                        if canceled.load(Ordering::Relaxed) {
                            kill_process_tree(&mut child).await;
                            update_task(&tasks, &task_id, |t| {
                                t.status = TaskStatus::Canceled;
                                t.speed_bps = 0;
                                t.eta_secs = 0;
                            });
                            Self::emit_dm(&app);
                            return;
                        }
                        if paused.load(Ordering::Relaxed) {
                            kill_process_tree(&mut child).await;
                            update_task(&tasks, &task_id, |t| {
                                t.status = TaskStatus::Paused;
                                t.speed_bps = 0;
                                t.eta_secs = 0;
                            });
                            Self::emit_dm(&app);
                            was_paused = true;
                            break;
                        }
                        let trimmed = line.trim();
                        let dest_opt = trimmed
                            .strip_prefix("[download] Destination: ")
                            .map(|s| s.trim())
                            .or_else(|| {
                                trimmed
                                    .strip_prefix("[Merger] Merging formats into \"")
                                    .and_then(|s| s.strip_suffix('"'))
                            })
                            .or_else(|| {
                                trimmed
                                    .strip_prefix("[download] ")
                                    .and_then(|s| s.strip_suffix(" has already been downloaded"))
                            });
                        if let Some(dest_str) = dest_opt {
                            let fname = PathBuf::from(dest_str)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("video.mp4")
                                .to_string();
                            update_task(&tasks, &task_id, |t| {
                                t.filename = fname;
                                t.save_path = dest_str.to_string();
                            });
                            Self::emit_dm(&app);
                        } else if trimmed.starts_with("[download]") && trimmed.contains('%') {
                            if let Some((pct, total_b, speed_b, eta_s)) =
                                parse_ytdlp_progress(trimmed)
                            {
                                update_task(&tasks, &task_id, |t| {
                                    if total_b > 0 {
                                        t.total_bytes = total_b;
                                        t.downloaded_bytes =
                                            ((total_b as f64) * pct / 100.0) as u64;
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

                if was_paused {
                    // Keep the same attempt index so resuming continues via `--continue`.
                    continue;
                }

                let err_msg = match stderr_handle {
                    Some(h) => h.await.unwrap_or_default(),
                    None => String::new(),
                };

                match child.wait().await {
                    Ok(s) if s.success() => {
                        succeeded = true;
                        break;
                    }
                    Ok(s) => {
                        last_error = if !err_msg.is_empty() {
                            err_msg
                        } else {
                            format!("yt-dlp berhenti dengan kode {}", s)
                        };
                    }
                    Err(e) => {
                        last_error = e.to_string();
                    }
                }
                attempt += 1;
            }

            if succeeded {
                update_task(&tasks, &task_id, |t| {
                    t.status = TaskStatus::Completed;
                    if t.total_bytes > 0 {
                        t.downloaded_bytes = t.total_bytes;
                    }
                    t.speed_bps = 0;
                    t.eta_secs = 0;
                    t.error = None;
                });
            } else if !canceled.load(Ordering::Relaxed) {
                update_task(&tasks, &task_id, |t| {
                    if t.status != TaskStatus::Canceled {
                        t.status = TaskStatus::Failed;
                        t.error = Some(last_error);
                        t.speed_bps = 0;
                    }
                });
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
            let mut slot = self.torrent_session.lock().await;
            if let Some(existing) = slot.as_ref() {
                existing.clone()
            } else {
                let created = Session::new(torrent_dir.clone())
                    .await
                    .map_err(|e| format!("Gagal memulai engine BitTorrent: {}", e))?;
                *slot = Some(created.clone());
                created
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

        let task = DownloadTaskDto::new(
            id.clone(),
            "torrent",
            name,
            magnet_or_url,
            torrent_dir.to_string_lossy().to_string(),
            TaskStatus::Downloading,
            0,
            "BitTorrent P2P".to_string(),
        );
        let (paused, canceled, notify) =
            self.register_task(&app, task, Some(rq_id), Some(handle.clone()));

        let tasks = self.tasks.clone();
        let task_id = id.clone();

        tauri::async_runtime::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(1));
            loop {
                tokio::select! {
                    biased;
                    _ = notify.notified() => {
                        if canceled.load(Ordering::Relaxed) {
                            break;
                        }
                    }
                    _ = ticker.tick() => {}
                }
                if canceled.load(Ordering::Relaxed) {
                    break;
                }
                let stats = handle.stats();
                let (down_bps, up_bps, peers, eta_s) = if let Some(live) = &stats.live {
                    let down = (live.download_speed.mbps * 1024.0 * 1024.0) as u64;
                    let up = (live.upload_speed.mbps * 1024.0 * 1024.0) as u64;
                    let peer_count = live.snapshot.peer_stats.live as u32;
                    let eta = if down > 0 && stats.total_bytes > stats.progress_bytes {
                        (stats.total_bytes - stats.progress_bytes) / down
                    } else {
                        0
                    };
                    (down, up, peer_count, eta)
                } else {
                    (0, 0, 0, 0)
                };

                let new_name = handle.name();
                let new_status = if stats.error.is_some() {
                    TaskStatus::Failed
                } else if paused.load(Ordering::Relaxed) {
                    TaskStatus::Paused
                } else if stats.finished {
                    TaskStatus::Completed
                } else {
                    TaskStatus::Downloading
                };

                update_task(&tasks, &task_id, |t| {
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
                    t.status = new_status;
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
        let handle = {
            let controls = self.controls.lock().unwrap();
            let ctrl = controls
                .get(id)
                .ok_or_else(|| "Task tidak ditemukan".to_string())?;
            ctrl.paused.store(true, Ordering::Relaxed);
            ctrl.notify.notify_waiters();
            ctrl.torrent_handle.clone()
        };

        if let Some(h) = handle {
            if let Some(session) = self.torrent_session.lock().await.as_ref() {
                let _ = session.pause(&h).await;
            }
        }
        update_task(&self.tasks, id, |t| {
            if matches!(t.status, TaskStatus::Downloading | TaskStatus::Queued) {
                t.status = TaskStatus::Paused;
                t.speed_bps = 0;
                t.eta_secs = 0;
            }
        });
        Self::emit_dm(&app);
        Ok(())
    }

    pub async fn resume_task(&self, app: AppHandle, id: &str) -> Result<(), String> {
        let handle = {
            let controls = self.controls.lock().unwrap();
            let ctrl = controls
                .get(id)
                .ok_or_else(|| "Task tidak ditemukan".to_string())?;
            ctrl.paused.store(false, Ordering::Relaxed);
            ctrl.notify.notify_waiters();
            ctrl.torrent_handle.clone()
        };

        if let Some(h) = handle {
            if let Some(session) = self.torrent_session.lock().await.as_ref() {
                let _ = session.unpause(&h).await;
            }
        }
        update_task(&self.tasks, id, |t| {
            if t.status == TaskStatus::Paused {
                t.status = TaskStatus::Downloading;
            }
        });
        Self::emit_dm(&app);
        Ok(())
    }

    pub async fn cancel_task(&self, app: AppHandle, id: &str) -> Result<(), String> {
        let torrent_id = {
            let controls = self.controls.lock().unwrap();
            controls.get(id).and_then(|ctrl| {
                ctrl.canceled.store(true, Ordering::Relaxed);
                ctrl.paused.store(false, Ordering::Relaxed);
                ctrl.notify.notify_waiters();
                ctrl.torrent_id
            })
        };

        if let Some(rq_id) = torrent_id {
            if let Some(session) = self.torrent_session.lock().await.as_ref() {
                let _ = session.delete(TorrentIdOrHash::Id(rq_id), false).await;
            }
        }
        update_task(&self.tasks, id, |t| {
            t.status = TaskStatus::Canceled;
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
        self.tasks.lock().unwrap().retain(|t| {
            matches!(
                t.status,
                TaskStatus::Downloading | TaskStatus::Queued | TaskStatus::Paused
            )
        });
        Self::emit_dm(&app);
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
    notify: Arc<Notify>,
) {
    update_task(&tasks, &id, |t| t.status = TaskStatus::Downloading);
    DownloadManager::emit_dm(&app);

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
                    update_task(&tasks, &id, |t| {
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

    update_task(&tasks, &id, |t| {
        t.total_bytes = total_bytes;
        t.connections = actual_conns;
        t.quality = if use_multi {
            format!("{}-Conn IDM", actual_conns)
        } else {
            "Direct Stream".to_string()
        };
    });
    DownloadManager::emit_dm(&app);

    let downloaded = Arc::new(AtomicU64::new(0));
    let stop_reporter = Arc::new(AtomicBool::new(false));
    let reporter = {
        let stop = stop_reporter.clone();
        let dl = downloaded.clone();
        let tasks = tasks.clone();
        let id = id.clone();
        let app = app.clone();
        let paused = paused.clone();
        tauri::async_runtime::spawn(async move {
            let mut prev: u64 = 0;
            let mut ticker = tokio::time::interval(Duration::from_millis(500));
            while !stop.load(Ordering::Relaxed) {
                ticker.tick().await;
                let cur = dl.load(Ordering::Relaxed);
                let delta = cur.saturating_sub(prev);
                prev = cur;
                let speed = if paused.load(Ordering::Relaxed) {
                    0
                } else {
                    delta * 2
                };
                let eta = if speed > 0 && total_bytes > cur {
                    (total_bytes - cur) / speed
                } else {
                    0
                };
                update_task(&tasks, &id, |t| {
                    t.downloaded_bytes = cur;
                    t.speed_bps = speed;
                    t.eta_secs = eta;
                });
                DownloadManager::emit_dm(&app);
            }
        })
    };

    let result = if use_multi {
        run_segmented_http(
            &client,
            &url,
            &save_path,
            total_bytes,
            actual_conns,
            &downloaded,
            &paused,
            &canceled,
            &notify,
        )
        .await
    } else {
        run_single_http(
            &client, &url, &save_path, &downloaded, &paused, &canceled, &notify,
        )
        .await
    };

    stop_reporter.store(true, Ordering::Relaxed);
    let _ = reporter.await;

    if canceled.load(Ordering::Relaxed) {
        let _ = tokio::fs::remove_file(&save_path).await;
        update_task(&tasks, &id, |t| {
            t.status = TaskStatus::Canceled;
            t.speed_bps = 0;
            t.eta_secs = 0;
        });
    } else {
        match result {
            Ok(()) => {
                let final_bytes = downloaded.load(Ordering::Relaxed);
                update_task(&tasks, &id, |t| {
                    t.status = TaskStatus::Completed;
                    t.downloaded_bytes = final_bytes;
                    if t.total_bytes == 0 {
                        t.total_bytes = final_bytes;
                    }
                    t.speed_bps = 0;
                    t.eta_secs = 0;
                });
            }
            Err(e) => update_task(&tasks, &id, |t| {
                t.status = TaskStatus::Failed;
                t.error = Some(e);
                t.speed_bps = 0;
                t.eta_secs = 0;
            }),
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
    notify: &Arc<Notify>,
) -> Result<(), String> {
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
    let mut handles = Vec::with_capacity(conns as usize);

    for idx in 0..conns {
        let start = (idx as u64) * chunk_size;
        let end = if idx == conns - 1 {
            total_bytes - 1
        } else {
            start + chunk_size - 1
        };
        let client = client.clone();
        let url = url.to_string();
        let path = save_path.to_path_buf();
        let downloaded = downloaded.clone();
        let paused = paused.clone();
        let canceled = canceled.clone();
        let notify = notify.clone();

        handles.push(tauri::async_runtime::spawn(async move {
            let mut file = OpenOptions::new()
                .write(true)
                .open(&path)
                .await
                .map_err(|e| e.to_string())?;
            let mut offset = start;

            while offset <= end {
                if wait_if_paused(&paused, &canceled, &notify).await {
                    return Ok(());
                }
                file.seek(SeekFrom::Start(offset))
                    .await
                    .map_err(|e| e.to_string())?;

                let req_fut = client
                    .get(&url)
                    .header(RANGE, format!("bytes={}-{}", offset, end))
                    .send();
                let mut resp = tokio::select! {
                    biased;
                    _ = notify.notified() => {
                        if canceled.load(Ordering::Relaxed) {
                            return Ok(());
                        }
                        continue;
                    }
                    r = req_fut => r.map_err(|e| e.to_string())?,
                };
                if !resp.status().is_success() {
                    return Err(format!("Segment {} HTTP {}", idx, resp.status()));
                }
                loop {
                    let chunk_opt = tokio::select! {
                        biased;
                        _ = notify.notified() => {
                            if canceled.load(Ordering::Relaxed) {
                                return Ok(());
                            }
                            if paused.load(Ordering::Relaxed) {
                                // Drop HTTP response immediately so network traffic stops at 0 B/s.
                                break;
                            }
                            continue;
                        }
                        c = resp.chunk() => c.map_err(|e| e.to_string())?,
                    };
                    let Some(bytes) = chunk_opt else {
                        break;
                    };
                    if canceled.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    file.write_all(&bytes).await.map_err(|e| e.to_string())?;
                    let n = bytes.len() as u64;
                    offset += n;
                    downloaded.fetch_add(n, Ordering::Relaxed);
                    if paused.load(Ordering::Relaxed) {
                        break;
                    }
                }
                file.flush().await.map_err(|e| e.to_string())?;
            }
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
    notify: &Arc<Notify>,
) -> Result<(), String> {
    if wait_if_paused(paused, canceled, notify).await {
        return Ok(());
    }
    let req_fut = client.get(url).send();
    let mut resp = tokio::select! {
        biased;
        _ = notify.notified() => {
            if canceled.load(Ordering::Relaxed) {
                return Ok(());
            }
            client.get(url).send().await.map_err(|e| e.to_string())?
        }
        r = req_fut => r.map_err(|e| e.to_string())?,
    };
    if !resp.status().is_success() {
        return Err(format!("HTTP error: {}", resp.status()));
    }
    let mut file = tokio::fs::File::create(save_path)
        .await
        .map_err(|e| e.to_string())?;
    loop {
        if wait_if_paused(paused, canceled, notify).await {
            return Ok(());
        }
        let chunk_opt = tokio::select! {
            biased;
            _ = notify.notified() => {
                if canceled.load(Ordering::Relaxed) {
                    return Ok(());
                }
                continue;
            }
            c = resp.chunk() => c.map_err(|e| e.to_string())?,
        };
        let Some(bytes) = chunk_opt else {
            break;
        };
        file.write_all(&bytes).await.map_err(|e| e.to_string())?;
        downloaded.fetch_add(bytes.len() as u64, Ordering::Relaxed);
    }
    file.flush().await.map_err(|e| e.to_string())?;
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
        let path = dir.join(name);
        if !path.exists() {
            return path;
        }
    }
    dir.join(format!("{}_{}", short_id(), safe))
}

fn extract_disposition_filename(header: &str) -> Option<String> {
    header.split(';').find_map(|part| {
        part.trim()
            .strip_prefix("filename=")
            .map(|rest| rest.trim_matches('"').trim())
            .filter(|val| !val.is_empty())
            .map(sanitize_filename)
    })
}

/// Parses yt-dlp progress line: `[download]  45.2% of ~120.50MiB at   5.20MiB/s ETA 00:12`
fn parse_ytdlp_progress(line: &str) -> Option<(f64, u64, u64, u64)> {
    let pct_idx = line.find('%')?;
    let pct: f64 = line[..pct_idx]
        .split_whitespace()
        .next_back()?
        .parse()
        .ok()?;

    let total_bytes = line
        .find(" of ")
        .and_then(|i| {
            line[i + 4..]
                .trim_start_matches('~')
                .split_whitespace()
                .next()
        })
        .map(parse_human_bytes)
        .unwrap_or(0);

    let speed_bps = line
        .find(" at ")
        .and_then(|i| line[i + 4..].split_whitespace().next())
        .map(|tok| parse_human_bytes(tok.trim_end_matches("/s")))
        .unwrap_or(0);

    let eta_secs = line
        .find(" ETA ")
        .and_then(|i| line[i + 5..].split_whitespace().next())
        .map(|tok| {
            let parts: Vec<u64> = tok.split(':').filter_map(|p| p.parse().ok()).collect();
            match parts.as_slice() {
                [m, s] => m * 60 + s,
                [h, m, s] => h * 3600 + m * 60 + s,
                _ => 0,
            }
        })
        .unwrap_or(0);

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

/// Strips noisy radio/playlist/tracking parameters from YouTube & X/Twitter URLs
/// so yt-dlp always targets the exact single video cleanly.
fn normalize_video_url(raw_url: &str) -> String {
    let Ok(parsed) = reqwest::Url::parse(raw_url.trim()) else {
        return raw_url.trim().to_string();
    };
    let host = parsed.host_str().unwrap_or("").to_lowercase();
    if host.contains("youtube.com") && parsed.path() == "/watch" {
        if let Some((_, vid)) = parsed.query_pairs().find(|(k, _)| k == "v") {
            return format!("https://www.youtube.com/watch?v={}", vid);
        }
    }
    if (host.contains("x.com") || host.contains("twitter.com"))
        && parsed.path().contains("/status/")
    {
        return format!("https://x.com{}", parsed.path());
    }
    raw_url.trim().to_string()
}
