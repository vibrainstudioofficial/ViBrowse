# 01 — Arsitektur Sistem, Pemilihan Library, & Tech Stack (V1.1.a)

Dokumen ini menjelaskan secara menyeluruh **mengapa setiap teknologi, engine, dan library/crate dipilih** dalam pembangunan **ViBrowse V1.1.a**, bagaimana arsitektur Multi-WebView bekerja, serta bagaimana ukuran installer berhasil ditekan hingga **2.45 MB** meskipun memuat browser modern + SQLite + Ad-Blocker + IDM 16-koneksi + klien BitTorrent penuh.

> Untuk pembahasan super detail mengenai perbandingan **WebView2 vs Chromium mentah (CEF) vs WebKitGTK (`libwebkit2gtk`) di Linux vs Android System WebView**, baca juga dokumen khusus: **[05-bedah-engine-webview-dan-lintas-platform.md](./05-bedah-engine-webview-dan-lintas-platform.md)**.

---

## 1. Filosofi Utama: Kenapa OS Native WebView + Rust (Bukan Electron / CEF)?

Tujuan utama ViBrowse adalah menghadirkan **kompatibilitas web setara Google Chrome** namun dengan **ukuran file sangat kecil dan pemakaian RAM sangat hemat**.

| Pendekatan | Ukuran Installer | Pemakaian RAM | Mesin Render | Alasan Diterima / Ditolak |
| --- | --- | --- | --- | --- |
| **Rust + Tauri v2 + OS WebView (Dipilih)** | **2.45 MB** (Setup) / **2.91 MB** (ZIP) | **Sangat Hemat** (*Shared OS DLL* + *Auto Tab Sleeping*) | **WebView2 (Chromium)** di Windows, **WebKitGTK** di Linux | **Dipilih**: Meminjam engine yang sudah ada di OS tanpa menggandakan biner 200 MB, backend 100% native Rust tanpa GC. |
| **Chromium Langsung / CEF / Electron** | ~140 – 220 MB | Tinggi (salinan Chromium + Node.js terpisah di RAM) | Chromium (bundled sendiri) | **Ditolak**: Aplikasi pihak ketiga tidak boleh meminjam `chrome.dll` milik Google Chrome, sehingga wajib mem-bundle `libcef.dll` ~180 MB sendiri dan wajib rilis ulang ratusan MB tiap ada patch keamanan. |
| **Pure Rust Browser dari Nol** | ~2 – 5 MB | Sangat rendah | Custom HTML/CSS parser | **Ditolak**: Membangun engine yang sanggup membuka YouTube, Gmail, React, WebGL, dan streaming modern membutuhkan ratusan *engineer-years*. |

---

## 2. Alasan Pemilihan Setiap Library & Crate di Tech Stack

Seluruh dependency di [Cargo.toml](../Cargo.toml) dipilih secara ketat—bahkan beberapa crate populer (`chrono`, `uuid`, `futures-util`, `rustls`) **sengaja dihapus/diganti** demi menjaga ukuran binary sekecil mungkin.

### 2.1 Layer Windowing & WebView
1. **`tauri` (`v2.12`, fitur: `wry`, `compression`, `common-controls-v6`, `dynamic-acl`, `x11`, `dbus`, `unstable`, `devtools`)**
   - **Kenapa dipakai**: Menyediakan jembatan IPC berkecepatan tinggi antara Rust dan UI, sistem izin keamanan (`capabilities`), kompresi Brotli otomatis untuk aset `ui/` (`compression`), serta fitur multi-webview (`unstable` -> `Window::add_child`) dalam satu jendela native.
   - **Kenapa `default-features = false`**: Agar kita memegang kendali penuh atas fitur yang diaktifkan tanpa membawa modul yang tidak dibutuhkan.
2. **`wry` & `tao` (di balik Tauri v2)**
   - **`tao`**: Library manajemen jendela lintas OS (Win32 di Windows, GTK3/X11/Wayland di Linux) buatan tim Tauri, jauh lebih ringan dibanding framework GUI berat seperti Qt.
   - **`wry`**: Abstraksi WebView lintas platform yang memetakan satu kode Rust ke **WebView2** (Windows), **WebKitGTK** (Linux), **WKWebView** (macOS/iOS), dan **Android System WebView** (Android).
3. **`webview2-com` (`v0.39`) & `windows-core` (`v0.62`) — Khusus Windows (`cfg(windows)`)**
   - **Kenapa dipakai**: Abstraksi standar Tauri belum mengekspos seluruh fitur tingkat rendah Chromium/WebView2. Dengan mengakses antarmuka COM `ICoreWebView2` secara langsung melalui `webview2-com`, ViBrowse mampu:
     - Mencegat **seluruh HTTP request subresource** (`<script>`, `<iframe>`, gambar iklan, tracker) lewat `WebResourceRequested` **sebelum paket menyentuh jaringan** (setara cara kerja uBlock Origin).
     - Menangkap sinyal HTML5 Video Fullscreen (`ContainsFullScreenElementChanged`) agar video langsung memenuhi 100% layar monitor.
4. **`webkit2gtk` (`v2.0`) — Khusus Linux (`cfg(target_os = "linux")`)**
   - **Kenapa dipakai**: Di Linux tidak ada WebView2 milik Microsoft; distro Linux menyediakan `libwebkit2gtk-4.1` berbasis WebKit. Crate `webkit2gtk` memungkinkan kita memasang listener sinyal GTK `connect_enter_fullscreen` dan `connect_leave_fullscreen` agar fullscreen video di Linux sama mulusnya dengan di Windows.

### 2.2 Layer Penyimpanan & Runtime Async
5. **`rusqlite` (`v0.32`, fitur: `bundled`)**
   - **Kenapa dipakai**: Menyimpan Bookmarks, History, Omnibox index, dan Settings dalam 1 file database lokal (`vibrowse.db`) dengan mode `WAL` (*Write-Ahead Logging*) yang sangat cepat dan tahan korupsi data.
   - **Kenapa fitur `bundled`**: Mengompilasi kode C asli SQLite langsung ke dalam `vibrowse.exe`, sehingga pengguna di PC lain **tidak perlu menginstal `sqlite3.dll` eksternal**.
6. **`tokio` (`v1`, fitur: `time`, `sync`, `fs`, `io-util`, `process`, `rt-multi-thread`)**
   - **Kenapa dipakai**: Runtime asynchronous standar industri di Rust. Menggerakkan 4 komponen secara konkuren tanpa membuat UI macet:
     1. Background timer **Auto Tab Sleeping** tiap 15 detik.
     2. Worker paralel **IDM Multi-Connection** (hingga 16 koneksi `Range` serentak per file).
     3. Pembacaan stream progress `stdout` dari proses `yt-dlp`.
     4. Engine jaringan P2P & DHT milik BitTorrent (`librqbit`).

### 2.3 Layer Download Manager (IDM + Video + BitTorrent) & Jaringan
7. **`reqwest` (`v0.12`, `default-features = false`, fitur: `default-tls`)**
   - **Kenapa dipakai**: HTTP client async andal untuk melakukan probing header (`HEAD` -> `Content-Length`, `Accept-Ranges`, `Content-Disposition`), mengunduh segmen paralel `Range: bytes=start-end`, serta mengunduh otomatis `yt-dlp`.
   - **Kenapa `default-tls` (Schannel/Native TLS) dan BUKAN `rustls-tls`?**:
     - `librqbit` sudah menggunakan `default-tls` (yang di Windows memanggil **Schannel** bawaan OS Windows dengan tambahan ukuran 0 KB!).
     - Saat sebelumnya `reqwest` memakai `rustls-tls`, kompiler terpaksa memasukkan **dua stack TLS sekaligus** (`rustls` + kriptografi assembly `ring` + sertifikat root `webpki-roots` ~800 KB). Menyatukan ke `default-tls` memangkas ratusan KB biner secara instan!
   - **Kenapa tanpa fitur `stream` / `futures-util`**: Method bawaan `resp.chunk().await` milik `reqwest` sudah sanggup membaca aliran byte secara bertahap tanpa perlu mengimpor crate `futures-util`.
8. **`librqbit` (`v8.1`, `default-features = false`, fitur: `default-tls`)**
   - **Kenapa dipakai**: Engine klien **BitTorrent / Magnet P2P 100% Native Rust** yang modern, cepat, mendukung DHT, tracker HTTP/UDP, magnet link (`magnet:?xt=...`), file `.torrent`, serta kontrol *pause/resume*.
   - **Kenapa `default-features = false`**: Mematikan fitur server HTTP API (`axum`), WebUI bawaan, dan UPnP media server milik `librqbit` yang tidak diperlukan di dalam ViBrowse sehingga ukuran binary tetap ramping.

### 2.4 Crate yang Sengaja Dihapus & Diganti `src/util.rs`
- **Tanpa `uuid`** -> Diganti fungsi `short_id()` (10 baris di [src/util.rs](../src/util.rs)) berbasis `AtomicU64` + `SystemTime` nanosecond mixing.
- **Tanpa `chrono`** -> Diganti fungsi `now_iso()` (20 baris algoritma *Howard Hinnant civil-from-days* di [src/util.rs](../src/util.rs)) untuk menghasilkan timestamp UTC `YYYY-MM-DDTHH:MM:SSZ` dengan nol dependency eksternal.
- **`url` (`v2`) & `dirs` (`v5`)**: Dipertahankan karena `url` zaten dipakai oleh `tauri`/`reqwest` (nol biaya tambahan di binary) untuk validasi domain & form encoding, sementara `dirs` ringan untuk mendeteksi path folder `Downloads` resmi pengguna di Windows maupun Linux.

---

## 3. Arsitektur Multi-WebView 3-Zona Disjoint

Di Windows (Win32) maupun Linux (GTK), menumpuk dua child WebView pada koordinat yang saling tumpang-tindih menyebabkan masalah **Z-order occlusion** (webview yang dibuat lebih dulu menutupi webview website).

ViBrowse menggunakan **Layout 3-Zona Disjoint** di `layout_webviews()` ([src/tabs.rs](../src/tabs.rs)):
1. **Mode Halaman Internal (`vibrowse://newtab`, `vibrowse://downloads`, `vibrowse://about`)**:
   - Webview `chrome` mengisi penuh `(0, 0, win_w, win_h)`; semua webview `tab-*` dan `panel` disembunyikan (`hide()`).
2. **Mode Website Aktif (`https://...`)**:
   - `chrome` diperkecil hanya setinggi Top Bar `(0, 0, win_w, top_h)` (`84px`, atau `114px` jika Bookmarks Bar aktif).
   - Webview tab aktif `tab-<id>` menempati `(0, top_h, win_w - side_w, win_h - top_h)`.
   - Jika Side Panel terbuka, webview khusus `panel` (`index.html?view=panel`) tampil di `(win_w - 360, top_h, 360, win_h - top_h)`.
3. **Mode True Monitor Fullscreen (Video Fullscreen / `F11`)**:
   - Jendela utama masuk mode `window.set_fullscreen(true)`, `chrome` dan `panel` disembunyikan, dan `tab-<id>` mengisi 100% layar monitor `(0, 0, monitor_w, monitor_h)`.

---

## 4. Teknik Optimasi Ukuran Binary (`9.53 MB` -> `6.33 MB`, Installer `2.45 MB`)

Kombinasi pengaturan di [Cargo.toml](../Cargo.toml) dan [.cargo/config.toml](../.cargo/config.toml):
```toml
[profile.release]
panic = "abort"     # Menghapus tabel stack unwinding
codegen-units = 1   # Memungkinkan optimasi lintas seluruh crate
lto = "fat"         # Full Link-Time Optimization di seluruh dependency tree
opt-level = "z"     # Instruksi mesin paling ringkas (lebih kecil dari "s" pada x86_64)
strip = "symbols"   # Menghapus seluruh simbol debug dari executable
```
Ditambah flag linker `-Wl,--gc-sections` untuk membuang fungsi C/Rust yang tidak pernah dipanggil (termasuk fungsi internal SQLite yang tidak dipakai), serta kompresi aset `ui/logo.png` dari `1.54 MB` menjadi `14.5 KB` (`128x128` RGBA).
