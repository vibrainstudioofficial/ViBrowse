# 05 — Referensi Variabel, Struktur Data, & Fungsi (V1.2a)

Dokumen ini menjadi peta cepat (*developer cheat sheet*) seluruh **konstanta, struktur data, variabel state, fungsi backend Rust (`src/`)**, **variabel & fungsi frontend (`ui/app.js`)**, serta **event IPC** yang dipakai pada **ViBrowse V1.2a** setelah refactoring.

---

## 1. Backend Rust (`src/`)

### 1.1 `src/util.rs` — Utilitas Ringan (Zero-Crate Pengganti `uuid` & `chrono`)

| Simbol | Jenis | Fungsi / Kegunaan |
| --- | --- | --- |
| `ID_COUNTER` | `static AtomicU64` | Penghitung sekuensial atomik untuk menjamin keunikan ID meski dibuat di nanodetik yang sama. |
| `short_id()` | `fn -> String` | Menghasilkan ID heksadesimal 8 karakter unik untuk tab, tugas download, dan popup terblokir. |
| `now_iso()` | `fn -> String` | Mengonversi waktu UTC saat ini ke format ISO-8601 (`YYYY-MM-DDTHH:MM:SSZ`) memakai algoritma *civil-from-days*. |
| `extract_host(raw_url)` | `fn -> Option<String>` | Mem-parsing URL dan mengembalikan nama host huruf kecil (mis. `youtube.com`). |

---

### 1.2 `src/state.rs` — State Global Browser & Snapshot IPC

#### Konstanta
| Konstanta | Nilai / Tipe | Kegunaan |
| --- | --- | --- |
| `NEWTAB_URL` | `"vibrowse://newtab"` | URL internal halaman Tab Baru (Speed Dial + Statistik). |
| `DOWNLOADS_URL` | `"vibrowse://downloads"` | URL internal dashboard Download Manager (IDM + Video + Torrent). |
| `ABOUT_URL` | `"vibrowse://about"` | URL internal halaman Tentang ViBrowse. |
| `HEADER_HEIGHT_BASE` | `84.0` | Tinggi header browser (tab strip + toolbar) tanpa Bookmarks Bar (px). |
| `BOOKMARKS_BAR_HEIGHT` | `30.0` | Tinggi tambahan Bookmarks Bar saat aktif (px). |
| `PANEL_WIDTH` | `360.0` | Lebar side panel kanan (Bookmarks, History, AdBlock, Downloads, Settings) dalam px. |
| `MAX_CLOSED_TABS` | `20` | Jumlah maksimal tab tertutup yang disimpan untuk `Ctrl+Shift+T`. |
| `MAX_BLOCKED_POPUPS` | `15` | Jumlah maksimal riwayat popup terblokir per tab. |
| `MAX_DETECTED_MEDIA` | `20` | Jumlah maksimal stream video/audio hasil *sniffer* per tab. |

#### Struktur Data & Fungsi
| Simbol | Jenis | Field Utama / Kegunaan |
| --- | --- | --- |
| `TabInfo` | `struct` | `id`, `title`, `url`, `is_loading`, `is_sleeping`, `pinned`, `incognito`, `zoom`, `last_active_ms`, `has_webview`, `blocked_popups`, `detected_media`. |
| `BlockedPopup` | `struct` | `id`, `url`, `host`, `timestamp` — catatan popup iklan yang dicegat pada tab. |
| `DetectedMedia` | `struct` | `url`, `kind` (`mp4`/`m3u8`/`webm`/dll.), `title` — media streaming yang ditangkap sniffer. |
| `ClosedTab` | `struct` | `url`, `title`, `incognito` — data tab yang baru ditutup untuk *Reopen Closed Tab*. |
| `InnerState` | `struct` | `tabs`, `active_tab_id`, `closed_tabs`, `open_panel`, `settings`, `fullscreen` — seluruh state mutable browser di dalam `Mutex`. |
| `InnerState::tab(id)` / `tab_mut(id)` | `method` | Helper pencarian tab berdasarkan ID secara ringkas (immutable & mutable). |
| `BrowserSnapshot` | `struct` | Payload JSON yang dikirim ke UI melalui event `browser-state` (`tabs`, `active_tab_id`, `open_panel`, `settings`, `adblock_blocked_count`, `active_tab_count`, `sleeping_tab_count`, `incognito_tab_count`, `fullscreen`). |
| `AppState` | `struct` | State terkelola Tauri (`inner: Mutex<InnerState>`, `storage: Storage`, `adblock: AdBlockEngine`, `downloader: DownloadManager`). |
| `tab_label(tab_id)` | `fn -> String` | Menghasilkan label child webview standar dengan format `tab-<id>`. |
| `is_internal_url(url)` | `fn -> bool` | Mengecek apakah URL termasuk `NEWTAB_URL`, `DOWNLOADS_URL`, atau `ABOUT_URL`. |
| `now_ms()` | `fn -> u64` | Timestamp UNIX saat ini dalam milidetik (untuk pelacakan *Auto Tab Sleeping*). |

---

### 1.3 `src/storage.rs` — Persistensi SQLite (WAL Mode)

| Simbol | Jenis | Field Utama / Kegunaan |
| --- | --- | --- |
| `Bookmark` | `struct` | `id`, `title`, `url`, `created_at`. |
| `HistoryEntry` | `struct` | `id`, `title`, `url`, `visit_count`, `last_visited`. |
| `OmniboxSuggestion` | `struct` | `title`, `url`, `kind` (`"bookmark"` / `"history"`). |
| `BrowserSettings` | `struct` | `search_engine`, `adblock_enabled`, `popup_blocker_enabled`, `tab_sleep_minutes`, `show_bookmarks_bar`, `theme` (`"dark"` / `"light"`). |
| `Storage` | `struct` | Pembungkus `Mutex<Connection>` SQLite dengan mode `PRAGMA journal_mode = WAL`. |
| `Storage::open(db_path)` | `method` | Membuka database lokal, mengaktifkan WAL, dan membuat tabel `bookmarks`, `history`, `settings` bila belum ada. |
| `add_bookmark` / `remove_bookmark_by_url` / `remove_bookmark_by_id` / `list_bookmarks` / `is_bookmarked` | `method` | Operasi CRUD lengkap untuk manajemen bookmark. |
| `record_visit` / `list_history` / `delete_history` / `clear_history` | `method` | Pencatatan kunjungan halaman (UPSERT `visit_count`), pencarian, dan penghapusan riwayat. |
| `search_omnibox(query, limit)` | `method` | Gabungan hasil pencarian cepat dari tabel `bookmarks` dan `history` untuk dropdown Omnibox. |
| `load_settings()` / `save_settings(&BrowserSettings)` | `method` | Membaca & menyimpan konfigurasi browser secara persisten. |

---

### 1.4 `src/adblock.rs` — Native Ad-Blocker & Smart Popup Blocker

| Simbol | Jenis | Kegunaan |
| --- | --- | --- |
| `AD_DOMAINS` | `const &[&str]` | Daftar 150+ domain jaringan iklan, tracker, pop-under streaming/donghua, dan shortlink. |
| `AD_URL_PATTERNS` | `const &[&str]` | Pola path URL iklan (`/popunder`, `/pop.js`, `vast.xml`, `invoke.js`, dll.). |
| `SAFE_POPUP_HOSTS` | `const &[&str]` | Whitelist domain login OAuth (Google, GitHub, Microsoft, Apple, Discord, Telegram) agar popup login tidak terblokir. |
| `AdBlockEngine` | `struct` | Menyimpan `blocked_domains` (`HashSet<&'static str>`), `blocked_domains_json` (precomputed), `allowlisted_sites`, `popup_allowlisted_sites`, `blocked_count` (`AtomicU64`), `enabled` (`AtomicBool`), `popup_blocker_enabled` (`AtomicBool`). |
| `set_enabled` / `is_enabled` / `set_popup_blocker_enabled` / `is_popup_blocker_enabled` | `method` | Toggle global Ad-Blocker dan Smart Popup Blocker. |
| `toggle_allowlist_site` / `is_site_allowlisted` | `method` | Menambah/menghapus pengecualian Ad-Blocker per domain situs. |
| `toggle_popup_allowlist_site` / `is_popup_site_allowlisted` | `method` | Menambah/menghapus pengecualian Popup Blocker per domain situs. |
| `is_blocked_host(host)` | `method` | Mengecek apakah host atau induk domainnya masuk daftar blokir `AD_DOMAINS`. |
| `should_block_subresource(url, page_host)` | `method` | Keputusan pemblokiran request tingkat WebView2 (`WebResourceRequested`). |
| `should_block_popup(target_url, source_host)` | `method` | Keputusan pemblokiran *new window* / *pop-under* lintas domain. |
| `build_init_script(enabled, popup_enabled)` | `method` | Menghasilkan skrip JS proteksi dalam halaman (filter `fetch`/`XHR`/`window.open`, CSS kosmetik iklan, penghapus overlay klik transparan). |

---

### 1.5 `src/downloader.rs` — Download Manager Terpadu (IDM + Video + BitTorrent)

#### Enum & Struktur Data
| Simbol | Jenis | Field Utama / Kegunaan |
| --- | --- | --- |
| `TaskStatus` | `enum` | `Queued`, `Running`, `Paused`, `Completed`, `Error`, `Canceled` (serialisasi huruf kecil via `serde`). |
| `DownloadTaskDto` | `struct` | Data tugas yang dikirim ke UI: `id`, `name`, `source`, `kind` (`http`/`video`/`torrent`), `status` (`TaskStatus`), `save_path`, `total_bytes`, `downloaded_bytes`, `speed_bps`, `eta_secs`, `connections`, `peers`, `error`, `created_at`. |
| `EngineStatusDto` | `struct` | Status ketersediaan `ytdlp_installed: bool`. |
| `TaskControl` | `struct` | Sinyal kontrol per tugas (`paused: AtomicBool`, `canceled: AtomicBool`, `torrent_id: Mutex<Option<usize>>`). |
| `DownloadManager` | `struct` | Pengelola utama semua tugas download, HTTP client `reqwest`, direktori simpan (`Downloads/ViBrowse`), dan sesi `librqbit` (lazy init). |

#### Fungsi & Method Utama
| Simbol | Jenis | Kegunaan |
| --- | --- | --- |
| `DownloadManager::new(data_dir)` | `method` | Inisialisasi folder unduhan, `reqwest::Client`, dan peta tugas. |
| `list_tasks()` / `engine_status()` | `method` | Mengambil daftar tugas terbaru dan status instalasi `yt-dlp`. |
| `start_http(app, url, filename, connections)` | `method` | Memulai unduhan HTTP/HTTPS akselerasi multi-segmen (hingga 16 koneksi paralel dengan `Range` request) + *fallback* single-stream. |
| `start_video(app, url, format)` | `method` | Menjalankan `yt-dlp` (atau otomatis memilih *direct download* bila URL langsung `.mp4`/`.webm`) untuk YouTube & 1000+ situs. |
| `start_torrent(app, source)` | `method` | Memulai unduhan P2P BitTorrent (`magnet:` atau file `.torrent`) lewat engine native `librqbit`. |
| `install_ytdlp(app)` | `method` | Mengunduh biner resmi `yt-dlp` dari GitHub Releases dengan 1 klik ke folder data lokal. |
| `pause_task` / `resume_task` / `cancel_task` / `remove_task` / `clear_finished` | `method` | Kontrol penuh siklus hidup tugas download (jeda, lanjutkan, batalkan, hapus, bersihkan selesai). |
| `register_task` / `update_task` / `wait_if_paused` / `emit_dm` | `helper` | Helper internal untuk registrasi tugas, pembaruan status/progres, penundaan saat paused, dan broadcast event `dm-updated`. |
| `ytdlp_bin_name()` / `sanitize_filename()` / `infer_filename_from_url()` | `helper` | Utilitas nama biner lintas OS dan sanitasi nama file hasil unduhan. |

---

### 1.6 `src/tabs.rs` — Multi-Tab Engine, Layout 3-Zona, Fullscreen & Auto Sleeper

| Simbol | Jenis | Kegunaan |
| --- | --- | --- |
| `emit_state(app)` | `fn` | Mem-broadcast `BrowserSnapshot` ke webview `chrome` dan `panel` lewat event `browser-state`. |
| `normalize_input_to_url(input, search_engine)` | `fn -> String` | Mengubah input Omnibox menjadi URL valid (mendukung `vibrowse://`, `http(s)://`, `magnet:`, domain lokal, atau query pencarian Google/DuckDuckGo/Bing). |
| `layout_webviews(app)` | `fn` | Menghitung ulang posisi & ukuran 3 zona disjoint (`chrome`, `tab-<id>` aktif, `panel`) serta menangani mode True Fullscreen. |
| `create_tab(app, initial_url, incognito, activate)` | `fn -> Result<TabInfo, String>` | Membuat tab baru (lazy webview untuk halaman internal, atau langsung spawn child webview untuk URL eksternal). |
| `switch_tab(app, tab_id)` | `fn` | Mengaktifkan tab tertentu, membangunkan tab jika sedang tidur (`is_sleeping`), dan memperbarui layout. |
| `close_tab(app, tab_id)` | `fn` | Menutup webview tab, menyimpannya ke `closed_tabs` (kecuali Incognito), dan memilih tab pengganti. |
| `reopen_closed_tab(app)` | `fn` | Membuka ulang tab terakhir yang ditutup (`Ctrl+Shift+T`). |
| `navigate_tab(app, tab_id, input)` | `fn` | Menavigasi tab ke URL baru (intersep otomatis `magnet:` ke Download Manager, menutup webview bila pindah ke halaman internal). |
| `sleep_tab(app, tab_id)` / `sleep_all_inactive_tabs(app)` | `fn` | Menutup child webview tab yang tidak aktif untuk membebaskan RAM 100% sambil menjaga metadata tab. |
| `set_browser_fullscreen(app, fullscreen)` / `toggle_browser_fullscreen(app)` | `fn` | Mengatur mode layar penuh jendela utama & menyembunyikan header chrome. |
| `start_tab_sleeper_loop(app)` | `fn` | Background worker Tokio (interval 15 detik) yang otomatis menidurkan tab idle melebihi `tab_sleep_minutes`. |
| `spawn_tab_webview(app, tab_id, raw_url, incognito)` | `fn` | Membangun child `WebviewBuilder` lengkap dengan *on_navigation*, *on_new_window* (Smart Popup Blocker), *on_document_title_changed*, serta hook native OS (WebView2 Ad-Blocker/Media Sniffer/Fullscreen di Windows, WebKitGTK Fullscreen di Linux). |
| `internal_title(url)` / `is_sleep_candidate(tab, active_id)` | `helper` | Helper penamaan judul halaman internal dan pengecekan kelayakan tab untuk ditidurkan. |

---

### 1.7 `src/commands.rs` & `src/main.rs` — 41 Tauri IPC Commands

Seluruh command didaftarkan di `src/main.rs` via `tauri::generate_handler![...]` dan diimplementasikan di `src/commands.rs`:

- **Tab & Navigasi (14)**: `get_state`, `cmd_new_tab`, `cmd_switch_tab`, `cmd_close_tab`, `cmd_reopen_closed_tab`, `cmd_navigate`, `cmd_tab_back`, `cmd_tab_forward`, `cmd_tab_reload`, `cmd_tab_zoom`, `cmd_toggle_pin_tab`, `cmd_sleep_tab`, `cmd_sleep_inactive_tabs`, `cmd_open_devtools`.
- **UI, Panel & Fullscreen (4)**: `cmd_set_panel`, `cmd_toggle_fullscreen`, `cmd_open_path`, `cmd_update_settings`.
- **Bookmarks, History & Omnibox (7)**: `cmd_list_bookmarks`, `cmd_toggle_bookmark`, `cmd_remove_bookmark`, `cmd_list_history`, `cmd_delete_history_entry`, `cmd_clear_history`, `cmd_search_omnibox`.
- **Ad-Blocker & Popup Blocker (5)**: `cmd_toggle_adblock_site`, `cmd_is_site_allowlisted`, `cmd_toggle_popup_site`, `cmd_is_popup_site_allowlisted`, `cmd_open_blocked_popup`.
- **Download Manager (11)**: `cmd_dm_list`, `cmd_dm_engine_status`, `cmd_dm_install_ytdlp`, `cmd_dm_start_http`, `cmd_dm_start_video`, `cmd_dm_start_torrent`, `cmd_dm_pause`, `cmd_dm_resume`, `cmd_dm_cancel`, `cmd_dm_remove`, `cmd_dm_clear_finished`.

---

## 2. Frontend UI (`ui/app.js`)

### 2.1 Konstanta & State Frontend

| Variabel / Konstanta | Jenis | Kegunaan |
| --- | --- | --- |
| `invoke` / `listen` | `const` | Akses langsung Tauri IPC (`window.__TAURI__.core.invoke` & `event.listen`). |
| `IS_PANEL` | `const bool` | Menandakan apakah instance `app.js` berjalan di webview `panel` (`?view=panel`) atau `chrome`. |
| `NEWTAB_URL`, `DOWNLOADS_URL`, `ABOUT_URL` | `const string` | Konstanta URL halaman internal yang selaras dengan `src/state.rs`. |
| `DEFAULT_SPEED_DIALS` | `const Array` | Daftar 6 pintasan situs default di halaman Tab Baru. |
| `ICONS` | `const Object` | Kumpulan string SVG vektor bergaya Lucide (tema, tab, download, kunci, pin, bintang, dll.). |
| `SHORTCUTS` | `const Array` | Daftar tabel shortcut keyboard Chrome yang ditampilkan di panel Settings. |
| `state` | `let Object` | Salinan terakhir `BrowserSnapshot` dari backend Rust. |
| `bookmarks` | `let Array` | Cache daftar bookmark lokal untuk Bookmarks Bar & tombol bintang Omnibox. |
| `dmTasks` | `let Array` | Cache daftar tugas Download Manager dari event `dm-updated`. |
| `dmEngine` | `let Object` | Status `ytdlp_installed` untuk badge engine di dashboard `vibrowse://downloads`. |
| `dmMode` / `dmFilter` | `let string` | Tab mode input (`"http"`/`"video"`/`"torrent"`) dan filter aktif (`"all"`/`"http"`/`"video"`/`"torrent"`) pada dashboard Download Manager. |
| `dom` | `const Object` | Referensi seluruh elemen DOM utama agar tidak melakukan `getElementById` berulang. |

### 2.2 Fungsi Utama Frontend

| Fungsi | Kegunaan |
| --- | --- |
| `activeTab()` / `isInternal(url)` / `withTab(fn)` | Helper mengakses tab aktif, mendeteksi halaman internal, dan mengeksekusi aksi jika tab aktif tersedia. |
| `applyTheme(theme)` | Menerapkan atribut `data-theme="light"` atau `"dark"` serta mengganti ikon tombol tema. |
| `saveSettings(patch)` | Menggabungkan perubahan parsial pengaturan dan mengirimnya ke `cmd_update_settings`. |
| `renderAll()` | Merender ulang seluruh komponen UI (`renderTabs`, `renderToolbar`, `renderBookmarksBar`, `renderNewTab`, `renderDownloadsPage`, `renderSidePanel`). |
| `renderTabs()` | Merender daftar tab (indikator loading, tidur, pin, incognito, tombol sleep & tutup). |
| `renderToolbar()` | Memperbarui teks Omnibox, ikon HTTPS/internal, tombol bookmark, pill AdBlock, pill Popup Terblokir, dan pill Media Sniffer. |
| `renderDownloadsPage()` | Merender dashboard penuh `vibrowse://downloads` (progres bar, kecepatan, ETA, peer torrent, tombol pause/resume/cancel/open folder). |
| `renderSidePanel()` | Memilih isi side panel kanan (`bookmarks`, `history`, `downloads`, `adblock`, `settings`). |
| `renderBookmarksPanel` / `renderHistoryPanel` / `renderDownloadsPanel` / `renderAdblockPanel` / `renderSettingsPanel` | Renderer modular untuk masing-masing tab side panel. |
| `formatBytes(n)` / `formatEta(secs)` / `createBadge(text)` | Utilitas pemformatan ukuran byte, estimasi waktu selesai, dan badge jumlah. |
| `bindEvents()` | Menghubungkan seluruh event klik toolbar, form Omnibox, shortcut keyboard (`Ctrl+T`, `Ctrl+W`, `Ctrl+L`, `Ctrl+J`, `F11`, `F12`, dll.), dan tombol dashboard. |
| `init()` | Titik masuk frontend: mengambil state awal (`get_state`, `cmd_list_bookmarks`, `cmd_dm_list`, `cmd_dm_engine_status`) dan berlangganan event `browser-state` & `dm-updated`. |

---

## 3. Event IPC (Backend -> Frontend)

| Nama Event | Sumber | Payload | Kegunaan |
| --- | --- | --- | --- |
| `browser-state` | `src/tabs.rs::emit_state` | `BrowserSnapshot` | Menyinkronkan perubahan tab, URL, panel, tema, jumlah blokir iklan, popup, media, dan status fullscreen secara real-time ke webview `chrome` & `panel`. |
| `dm-updated` | `src/downloader.rs::emit_dm` | `Vec<DownloadTaskDto>` | Memperbarui progres, kecepatan unduh, ETA, dan jumlah peer BitTorrent secara live di `vibrowse://downloads` & panel Downloads. |
