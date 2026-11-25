# 02 — Spesifikasi Fitur & Detail Implementasi (V1.2a)

Dokumen ini membedah secara mendetail seluruh fitur yang tersedia di **ViBrowse V1.2a**, cara kerjanya di backend Rust & frontend UI, serta daftar lengkap seluruh **41 Tauri IPC commands** (lihat juga peta simbol lengkap di [05-referensi-variabel-dan-fungsi.md](./05-referensi-variabel-dan-fungsi.md)).

---

## 1. Multi-Tab Engine & Auto Tab Sleeping (RAM Saver)

**File terkait**: [src/tabs.rs](../src/tabs.rs), [src/state.rs](../src/state.rs), [ui/app.js](../ui/app.js)

- **Lazy WebView Creation**: Tab yang berada di halaman internal (`vibrowse://newtab`, `vibrowse://downloads`, `vibrowse://about`) tidak membuat instance WebView eksternal (`has_webview = false`), sehingga membuka tab internal hampir tidak memakan RAM tambahan.
- **Auto Tab Sleeping**:
  - Background Tokio worker `start_tab_sleeper_loop()` memeriksa tab tidak aktif setiap **15 detik**.
  - Tab non-pin yang menganggur melebihi batas `tab_sleep_minutes` (1m / 5m / 15m / 30m) otomatis ditutup WebView-nya (`wv.close()`) sehingga RAM dibebaskan 100%, sambil tetap mempertahankan judul, URL, dan posisi tab dengan ikon bulan sabit (`sleep`).
  - Klik tab tidur otomatis membangunkan ulang halaman tersebut.
- **Kontrol Cepat**: Tombol sleep per tab, pill **RAM Saver** di kanan atas untuk menidurkan semua tab tidak aktif sekaligus, klik kanan tab untuk **Pin Tab**, klik tengah untuk tutup tab, dan `Ctrl+Shift+T` untuk membuka kembali hingga 20 tab terakhir yang ditutup.

---

## 2. Native Ad-Blocker & Smart Popup Blocker (dengan Pengecualian)

**File terkait**: [src/adblock.rs](../src/adblock.rs), [src/tabs.rs](../src/tabs.rs)

1. **Lapis 1 — Native Subresource Filter (`WebResourceRequested` di Windows)**:
   - Mencegat request `<script>`, `<iframe>`, gambar iklan, dan tracker langsung di level COM `ICoreWebView2` sebelum keluar ke jaringan (mengembalikan HTTP `204` kosong).
   - Mencakup **150+ domain jaringan iklan & pop-under situs streaming/donghua** (DoubleClick, GoogleSyndication, Adsterra, Monetag, PropellerAds, HilltopAds, Clickadu, AdCash, ExoClick, PopAds, PopCash, TrafficStars, Galaksion, Histats, shortlink redirector, dll.) + pencocokan pola URL (`/popunder`, `/pop.js`, `vast.xml`, `invoke.js`, dll.).
2. **Lapis 2 — In-Page Protection & Anti Click-Overlay**:
   - Memfilter `fetch()`, `XMLHttpRequest`, `window.open()`, menyembunyikan kontainer iklan via CSS kosmetik, serta menghapus elemen transparan ber-`z-index >= 9999` penutup layar yang biasa membajak klik pertama pada player video.
3. **Smart Popup Blocker + Pengecualian Fleksibel**:
   - Di `on_new_window`, ViBrowse otomatis memblokir tab baru yang mengarah ke domain iklan maupun popup lintas domain (*cross-origin pop-under*), sambil tetap mengizinkan domain login aman (Google, GitHub, Microsoft, Apple, Discord, Telegram).
   - Ketika popup diblokir, tombol kuning **`Popup`** muncul di toolbar. Pengguna dapat memilih **Buka Sekali** atau **Kecualikan & Buka** (mengizinkan situs tersebut seterusnya).
   - Panel **Ad & Popup Blocker** menyediakan toggle terpisah untuk mengecualikan situs aktif dari **Ad-Blocker** maupun dari **Popup Blocker**.

---

## 3. Unified Download Manager (IDM + Video/YouTube + Native BitTorrent)

**File terkait**: [src/downloader.rs](../src/downloader.rs), [ui/index.html](../ui/index.html), [ui/app.js](../ui/app.js)

1. **IDM Multi-Connection HTTP/HTTPS Accelerator**:
   - Memeriksa dukungan `Accept-Ranges: bytes` dan `Content-Length`. Untuk file >= 1 MB, file dialokasikan di disk lalu diunduh paralel menggunakan **4, 8, atau 16 koneksi `Range` serentak** (otomatis fallback ke single stream jika server tidak mendukung range).
   - Klik unduhan biasa di browser (`on_download`) otomatis dialihkan ke akselerator 8-koneksi ini.
   - Mendukung **Pause, Resume, Cancel, Remove**, kecepatan live, dan perhitungan ETA.
2. **Video / Media Sniffer Otomatis**:
   - Hook jaringan otomatis mendeteksi URL media (`.mp4`, `.m3u8` HLS, `.webm`, `.mkv`, `.mp3`, `.m4a`) serta halaman platform video populer (YouTube, TikTok, Instagram, Facebook, Bilibili, Vimeo, Twitch, X/Twitter).
   - Tombol hijau **`N Media`** muncul otomatis di toolbar untuk mengunduh video dari halaman aktif dalam 1 klik.
3. **YouTube & 1000+ Situs Video Downloader (`yt-dlp`)**:
   - Pilihan kualitas: **Best MP4**, **1080p MP4**, **720p MP4**, **480p MP4**, dan **Audio Only (M4A)**.
   - Dilengkapi tombol **Install yt-dlp (1-Klik)** di dalam Dashboard yang mengunduh biner resmi (`yt-dlp.exe` di Windows / `yt-dlp_linux` + `chmod 755` di Linux) langsung ke folder data aplikasi tanpa jendela console.
4. **Native Rust BitTorrent / Magnet Client (`librqbit 8.1`)**:
   - Mendukung tautan `magnet:?xt=...`, URL `.torrent`, maupun path file `.torrent` lokal dengan DHT & P2P aktif.
   - Klik tautan `magnet:` di halaman web mana pun otomatis ditangkap dan langsung diunduh ke folder `Downloads/ViBrowse Torrents`.
   - Menampilkan kecepatan unduh (`↓`), unggah (`↑`), jumlah **peers aktif**, progress bar, dan kontrol Pause/Resume/Cancel di Dashboard **`vibrowse://downloads`**.

---

## 4. True Monitor Fullscreen, Light/Dark Mode, Modern SVG UI, & About Page

- **True Monitor Fullscreen**: Menangkap event fullscreen HTML5 baik di Windows (`ContainsFullScreenElementChanged`) maupun Linux (`WebKitGTK connect_enter_fullscreen`), menyembunyikan seluruh top bar/panel, dan memperluas video 100% memenuhi monitor (juga mendukung tombol **`F11`**).
- **Light Mode & Dark Mode**: Tersimpan persisten di SQLite (`BrowserSettings.theme`), dapat diubah instan lewat ikon **Sun / Moon** di toolbar atau dari panel Settings.
- **Modern Lucide SVG Icons**: Seluruh ikon UI menggunakan vektor SVG tajam tanpa ketergantungan font/CDN eksternal.
- **About ViBrowse (`vibrowse://about`)**: Menampilkan versi **`V1.1.a`**, daftar fitur lengkap, rincian engine, serta statistik sesi real-time.
- **Optimasi Snapshot O(1)**: `AppState::snapshot()` mengambil himpunan URL bookmark dalam 1 query tunggal (`bookmarked_urls_set()`) sehingga bebas dari masalah N+1 SQLite query.

---

## 5. Referensi Lengkap 39 Tauri IPC Commands (`src/commands.rs`)

| Kategori | Command | Mode | Fungsi Utama |
| --- | --- | --- | --- |
| **State & Window** | `get_state` | sync | Mengambil `BrowserSnapshot` terbaru |
| | `cmd_set_panel` | async | Membuka/menutup side panel kanan |
| | `cmd_toggle_fullscreen` | async | Toggle fullscreen monitor (`F11`) |
| | `cmd_open_devtools` | sync | Membuka/menutup DevTools (`F12`) |
| | `cmd_open_path` | sync | Membuka file / folder (`explorer` di Windows, `xdg-open` di Linux) |
| **Tab & Navigasi** | `cmd_new_tab` | async | Membuat tab reguler atau Incognito |
| | `cmd_switch_tab` | async | Berpindah tab & membangunkan tab tidur |
| | `cmd_close_tab` | async | Menutup tab & menyimpan ke riwayat tutup |
| | `cmd_reopen_closed_tab` | async | Membuka ulang tab terakhir yang ditutup (`Ctrl+Shift+T`) |
| | `cmd_navigate` | async | Navigasi URL, query search, atau `vibrowse://*` |
| | `cmd_tab_back` / `forward` / `reload` | sync | Navigasi riwayat halaman & muat ulang |
| | `cmd_tab_zoom` | sync | Mengatur level zoom (`0.5x` – `2.5x`) |
| | `cmd_toggle_pin_tab` | sync | Pin / unpin tab |
| | `cmd_sleep_tab` / `cmd_sleep_inactive_tabs` | async | Menidurkan satu atau semua tab menganggur |
| **Bookmarks & History** | `cmd_list_bookmarks` | sync | Mengambil semua bookmark |
| | `cmd_toggle_bookmark` / `cmd_remove_bookmark` | sync | Menambah atau menghapus bookmark |
| | `cmd_list_history` | sync | Mencari riwayat kunjungan (maks 150) |
| | `cmd_delete_history_entry` / `cmd_clear_history` | sync | Menghapus item atau seluruh history |
| | `cmd_search_omnibox` | sync | Autocomplete gabungan Bookmarks + History |
| **Settings & Blocker** | `cmd_update_settings` | async | Menyimpan pengaturan (termasuk tema Light/Dark) |
| | `cmd_toggle_adblock_site` / `cmd_is_site_allowlisted` | sync | Pengecualian Ad-Blocker per domain |
| | `cmd_toggle_popup_site` / `cmd_is_popup_site_allowlisted` | sync | Pengecualian Popup Blocker per domain |
| | `cmd_open_blocked_popup` | async | Membuka popup yang diblokir (sekali / kecualikan situs) |
| **Download Manager** | `cmd_dm_list` / `cmd_dm_engine_status` | sync | Daftar tugas unduhan & status kesiapan engine |
| | `cmd_dm_install_ytdlp` | async | Auto-download biner resmi `yt-dlp` (Windows/Linux) |
| | `cmd_dm_start_http` | async | Mulai unduhan IDM multi-koneksi (1–16 koneksi) |
| | `cmd_dm_start_video` | async | Mulai unduhan video YouTube/1000+ situs via `yt-dlp` |
| | `cmd_dm_start_torrent` | async | Mulai unduhan Magnet / `.torrent` via `librqbit` |
| | `cmd_dm_pause` / `resume` / `cancel` | async | Jeda, lanjutkan, atau batalkan unduhan |
| | `cmd_dm_remove` / `cmd_dm_clear_finished` | sync | Hapus item atau bersihkan unduhan selesai |
