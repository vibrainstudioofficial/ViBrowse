# Changelog

Seluruh perubahan penting pada proyek **ViBrowse** dicatat di dokumen ini.

## [V1.2.4] — Fix Download Pause/Stop, Custom DNS (DoH), & Import/Export Bookmarks

### Perbaikan & Fitur Baru
- **Fix Tombol Pause & Stop Download Manager**:
  - Memperbaiki Pause & Stop pada **IDM HTTP Multi-Koneksi** dengan `tokio::select!` + `Notify`: klik Pause kini benar-benar menutup koneksi TCP (0 B/s) lalu melanjutkan tepat dari byte offset (`Range`) saat Resume; klik Stop langsung menghentikan unduhan & menghapus file parsial seketika.
  - Memperbaiki Pause & Stop pada **Video Downloader (`yt-dlp`)**: menambahkan `--continue` untuk Pause/Resume nyata serta terminasi process tree instan (`taskkill /F /T /PID` di Windows) saat Stop.
- **Custom DNS (DNS-over-HTTPS / DoH)**: Pilihan DNS terenkripsi native WebView2 di panel **Settings** dengan preset **Cloudflare (`1.1.1.1`)**, **Google (`8.8.8.8`)**, **AdGuard DNS**, **Quad9**, dan **Custom DoH URL** (NextDNS, dll.) lengkap dengan tombol **Simpan & Restart ViBrowse**.
- **Import & Export Bookmarks**:
  - Auto-detect 1-klik dari browser terpasang (**Google Chrome**, **Microsoft Edge**, **Brave**, **Vivaldi**, **Opera**) di Windows & Linux.
  - Import dari file standar **Netscape Bookmark HTML (`bookmarks.html`)** (Firefox/Chrome/Edge/Safari) serta tombol **Export HTML** di panel Bookmarks.

---

## [V1.2.3] — Fix Google Drive/Docs/Sheets, Linux Mint GTK3 Maximize & WebKitGTK Repaint

### Perbaikan Bug
- **Fix Google Drive, Google Sheets, & Google Docs (Windows & Linux)**: Melindungi seluruh host Google Workspace (`drive/docs/sheets/slides/forms/calendar/mail/meet/apis/clients*/gstatic/googleusercontent`) dari intersepsi `adblock-rust`, cosmetic filtering, dan JS fetch/XHR patch, serta mengizinkan alur `window.open("about:blank")` saat membuka dokumen dari Drive.
- **Fix Address Bar Tertimpa URL Iframe/Tracker**: Memindahkan pembaruan `tab.url` dari `on_navigation` (yang ikut terpanggil oleh `<iframe>` di WebKitGTK) ke `on_page_load` (khusus main frame).
- **Fix Linux Mint 22.2 / Ubuntu 24.04 Window Lock & Start Page Overlap**:
  - Menetapkan `WEBKIT_DISABLE_DMABUF_RENDERER=1` secara otomatis di Linux untuk mencegah glitch repaint/compositing WebKitGTK 4.1.
  - Memperbaiki ukuran awal jendela agar ramah layar laptop `1366×768` (`1180×680`, min `640×420`) serta menerapkan `gtk_window.set_geometry_hints(MIN_SIZE)` + `queue_resize/queue_draw` agar `GtkFixed` tidak mengunci resize maupun tombol Maximize.
  - Menyembunyikan `#workspace` & `#newtab-page` di DOM saat tab web eksternal aktif sehingga Start Page tidak pernah menindih halaman web.
- **Migrasi Repositori Resmi GitHub**: Memindahkan repositori resmi ke `https://github.com/vibrainstudioofficial/ViBrowse`.

---

## [V1.2.2] — Upgrade Ad-Blocker ke Brave `adblock-rust` + Auto-Sync Filter List

### Peningkatan Utama
- **Ad-Blocker Engine v2 (`Brave adblock-rust`)**: Mengganti daftar blokir statis dengan engine **`adblock-rust`** milik Brave untuk pemblokiran iklan, tracker, pop-under, dan elemen kosmetik yang jauh lebih akurat dan menyeluruh.
- **Auto-Sync Filter List Komunitas (100.000+ Aturan)**: Sinkronisasi otomatis di latar belakang tiap 24 jam (plus tombol **Sync Filter List** manual di panel AdBlock) untuk **EasyList**, **EasyPrivacy**, **ABPindo** (iklan & banner situs Indonesia), serta **Peter Lowe's Ad & Tracker List** dengan cache biner cepat (`adblock_engine.dat`).
- **Cosmetic Filtering Dinamis & YouTube Ad-Stripper**: Menyembunyikan elemen iklan per-situs secara otomatis (Windows & Linux) serta mempercepat/melewati iklan video YouTube & menyembunyikan banner anti-adblock.
- **Catatan Ukuran File**: Sebagai konsekuensi integrasi engine `adblock-rust` + database Public Suffix List (`psl`), ukuran file binary/installer mengalami sedikit penambahan ukuran agar kualitas pemblokiran iklan jauh lebih baik.

---

## [V1.2.1] — Fix Google Logout, Clear Cache & Cookies, Profile Management, & Auto Update

### Perbaikan & Fitur Baru
- **Fix Login/Logout Google & YouTube**: Menambahkan seluruh domain autentikasi Google/YouTube (`accounts.youtube.com`, `myaccount.google.com`, `mail.google.com`, `ogs.google.com`, `google.com`, `google.co.id`, `youtube.com`) ke whitelist popup agar alur logout Google/YouTube tidak terblokir.
- **Clear Cache & Cookies (1-Klik Logout Semua Akun)**: Fitur pembersihan cache, cookies, localStorage, dan sesi login aktif secara native (`Webview::clear_all_browsing_data`) melalui panel **Profiles & Privacy** dan **Settings**.
- **Profile Management (Multi-Profil Terisolasi)**: Buat, pilih, dan hapus profil browser terpisah (dengan warna custom & folder `data_directory` terisolasi untuk cookies/login masing-masing profil) langsung dari toolbar.
- **Default Browser Check & Auto Update**: Banner pengecekan Default Browser ala Chrome, dukungan argumen CLI URL, serta deteksi update otomatis via GitHub Releases + fallback `https://browse.vibrain.id/version.json`.
- **Dual Mirror Download**: Seluruh paket Windows (`.exe`, `.zip`), Linux (`.deb`, `.rpm`), dan Source Code (`.zip`) tersedia sekaligus di **Cloudflare Edge (`browse.vibrain.id`)** dan **GitHub Releases**.

---

## [V1.2a] — Refactoring Kode, Dokumentasi Referensi Variabel & Fungsi

### Peningkatan & Refactoring
- **Pembersihan Kode & Penghapusan Redundansi**:
  - Menambahkan `TaskStatus` enum bertipe kuat, `DownloadTaskDto::new()`, `register_task()`, `update_task()`, dan `wait_if_paused()` di `src/downloader.rs` sehingga eliminasi duplikasi mutasi status download dan penyeragaman `tauri::async_runtime::spawn`.
  - Menghapus `RwLock` yang tidak perlu pada `blocked_domains` di `src/adblock.rs` serta prekomputasi JSON domain blokir satu kali saat startup.
  - Menambahkan helper `InnerState::tab()`, `InnerState::tab_mut()`, `tab_label()` di `src/state.rs` serta menyederhanakan loop layout di `src/tabs.rs` dan command di `src/commands.rs`.
  - Menghapus field/parameter tak terpakai (`homepage` di `BrowserSettings`, `folder` di `add_bookmark`, `ICONS.check` di `ui/app.js`).
  - Merapikan `ui/app.js` dengan helper `isInternal()`, `withTab()`, `createBadge()`, `saveSettings()`, `SHORTCUTS`, dan binding panel berbasis `data-panel`.
  - Menambahkan `/gen/` ke `.gitignore` agar artefak build Tauri tidak ikut ter-commit.
- **Dokumentasi Baru & Perbaikan GitHub**:
  - Menambahkan **[05-referensi-variabel-dan-fungsi.md](dokumen/05-referensi-variabel-dan-fungsi.md)** berisi peta lengkap konstanta, struct/enum, variabel, fungsi Rust & JS, serta event IPC.
  - Merapikan urutan dokumentasi `dokumen/01` s.d. `05` dan memperbaiki tampilan logo serta tautan rilis di `README.md`.

---

## [V1.1.a] — Rilis Publik Perdana

### Fitur Utama
- **Multi-Tab Browser & Auto Tab Sleeping (RAM Saver)**: Unload otomatis tab tidak aktif di latar belakang untuk membebaskan RAM, didukung Pin Tab, Sleep manual, dan Reopen Closed Tab (`Ctrl+Shift+T`).
- **Native Ad-Blocker & Smart Popup Blocker**: Pemblokiran subresource iklan/tracker di level WebView2 (`WebResourceRequested`), netralisasi klik-overlay transparan, serta pemblokir *pop-under* lintas domain dengan opsi pengecualian per situs.
- **Unified Download Manager (`vibrowse://downloads`)**:
  - Akselerator unduhan HTTP/HTTPS hingga **16 koneksi paralel** ala IDM.
  - **Video / Media Sniffer** otomatis di toolbar untuk stream `.mp4`, `.m3u8` (HLS), `.webm`, `.mp3`, dan `.m4a`.
  - Integrasi **YouTube & 1000+ platform video** melalui `yt-dlp` dengan tombol *1-Klik Auto-Install*.
  - Klien **BitTorrent / Magnet P2P Native Rust** (`librqbit 8.1`) dengan intersep otomatis tautan `magnet:`.
- **Mode Incognito (`Ctrl+Shift+N`)**: Penjelajahan privat terisolasi tanpa pencatatan riwayat.
- **Penyimpanan Lokal SQLite WAL**: Bookmarks, Bookmarks Bar (`Ctrl+Shift+B`), History, Omnibox autocomplete, dan pengaturan persisten.
- **True Monitor Fullscreen**: Dukungan penuh HTML5 video fullscreen satu layar penuh di Windows (`WebView2`) & Linux (`WebKitGTK`), ditambah shortcut `F11`.
- **Modern SVG UI + Light & Dark Mode**: Ikon vektor tajam bergaya Lucide, peralihan tema terang/gelap instan, serta halaman informasi **`vibrowse://about`**.
- **Optimasi Ukuran Binary Ekstrem**: Installer Windows hanya **2.45 MB** dan versi Portable ZIP **2.91 MB**, dilengkapi builder lintas platform Linux (`.deb` & `.rpm`).
