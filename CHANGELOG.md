# Changelog

Seluruh perubahan penting pada proyek **ViBrowse** dicatat di dokumen ini.

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
