<div align="center">
  <img src="https://raw.githubusercontent.com/fransetiaone/ViBrowse/main/ui/logo.webp" alt="ViBrowse Logo" width="96" height="96" />
  <h1>ViBrowse</h1>
  <p><strong>Browser Desktop Modern yang Ultra-Ringan (~2.45 MB), Hemat RAM, &amp; Kompatibel Setara Google Chrome</strong></p>
  <p>
    <a href="https://github.com/fransetiaone/ViBrowse/releases/latest"><img src="https://img.shields.io/badge/Release-V1.2a-2563eb?style=for-the-badge" alt="Version V1.2a" /></a>
    <img src="https://img.shields.io/badge/Built%20With-Rust%20%2B%20Tauri%20v2-f97316?style=for-the-badge" alt="Rust + Tauri v2" />
    <a href="LICENSE"><img src="https://img.shields.io/badge/License-GPL--3.0-10b981?style=for-the-badge" alt="GPL-3.0 License" /></a>
  </p>
</div>

---

## Mengapa ViBrowse?

**ViBrowse** dibangun dari nol menggunakan **Rust + Tauri v2** dengan memanfaatkan engine WebView bawaan Sistem Operasi (**WebView2 / Chromium** di Windows dan **WebKitGTK** di Linux). Hasilnya: **100% kompatibilitas web modern** tanpa harus membundel biner Chromium 200 MB milik sendiri.

- **Ukuran Ekstrem Kecil**: Installer Windows hanya **~2.45 MB** (`Portable.zip` **~2.91 MB**).
- **Auto Tab Sleeping (RAM Saver)**: Tab tidak aktif otomatis di-*unload* oleh background worker Tokio untuk membebaskan RAM hingga 100% tanpa kehilangan posisi maupun URL tab.
- **Native Ad-Blocker & Smart Popup Blocker**: Mencegat `<script>`, `<iframe>`, tracker di level engine sebelum keluar ke jaringan + pemblokir *pop-under* lintas domain untuk situs streaming/donghua (dengan fitur pengecualian per situs).
- **Download Manager Terpadu (IDM + Video + BitTorrent)**:
  - **IDM Accelerator**: Unduhan HTTP/HTTPS terbagi hingga **16 segmen koneksi paralel** (`Range` requests) dengan *Pause/Resume*.
  - **Media / Video Sniffer**: Otomatis mendeteksi stream `.mp4` / `.m3u8` (HLS) di halaman aktif.
  - **YouTube & 1000+ Situs**: Integrasi `yt-dlp` dengan tombol *1-Klik Auto-Install* langsung dari dalam browser.
  - **Native Rust BitTorrent (`librqbit`)**: Klien P2P/DHT tertanam langsung untuk tautan `magnet:` dan file `.torrent`.
- **Privasi & Kenyamanan Penuh**: Mode Incognito (`InPrivate`), penyimpanan lokal SQLite WAL, True Monitor Fullscreen (`F11` & HTML5 video), DevTools (`F12`), Chrome Keyboard Shortcuts, serta **Dark Mode & Light Mode** dengan ikon vektor SVG modern.

---

## Unduh Rilis Resmi (V1.2a)

Unduh paket siap pakai melalui halaman **[GitHub Releases](https://github.com/fransetiaone/ViBrowse/releases/latest)**:

| Platform | Paket | Ukuran | Keterangan |
| --- | --- | --- | --- |
| **Windows 10 / 11 (x64)** | `ViBrowse-V1.2a-Setup.exe` | **~2.45 MB** | Installer otomatis (Desktop & Start Menu shortcut, Uninstaller, deteksi otomatis WebView2, tanpa butuh Admin/UAC) |
| **Windows 10 / 11 (x64)** | `ViBrowse-V1.2a-Portable.zip` | **~2.91 MB** | Ekstrak & langsung jalankan `vibrowse.exe` |
| **Linux Debian / Ubuntu (x64)** | `ViBrowse-V1.2a-linux-amd64.deb` | ~3–5 MB | Untuk Ubuntu, Debian, Linux Mint, Pop!_OS (`sudo apt install ./ViBrowse-*.deb`) |
| **Linux Fedora / RHEL (x64)** | `ViBrowse-V1.2a-linux-x86_64.rpm` | ~3–5 MB | Untuk Fedora, RHEL, openSUSE, Rocky (`sudo dnf install ./ViBrowse-*.rpm`) |

---

## Dokumentasi Teknis Lengkap

Seluruh pembahasan arsitektur, pemilihan library, cara kerja engine, hingga referensi variabel & fungsi tersedia di folder [`dokumen/`](dokumen/README.md):

1. [**Arsitektur Sistem & Tech Stack**](dokumen/01-arsitektur-dan-tech-stack.md) — Alasan pemilihan setiap crate Rust, layout Multi-WebView 3-Zona Disjoint, dan optimasi ukuran binary.
2. [**Spesifikasi Fitur & Implementasi**](dokumen/02-fitur-dan-implementasi.md) — Bedah teknis semua modul & referensi lengkap 41 Tauri IPC Commands.
3. [**Panduan Build & Pengembangan**](dokumen/03-panduan-build-dan-pengembangan.md) — Cara kompilasi lokal di Windows & Linux (`.deb` / `.rpm`) serta roadmap fitur.
4. [**Bedah Mendalam WebView2, WebKitGTK (`libwebkit2gtk`), CEF & Lintas Platform**](dokumen/04-bedah-engine-webview-dan-lintas-platform.md) — Mengapa memakai OS WebView dibanding mem-bundle Chromium mentah, serta kesiapan Windows, Linux, dan Android.
5. [**Referensi Variabel, Struktur Data, & Fungsi**](dokumen/05-referensi-variabel-dan-fungsi.md) — Peta lengkap konstanta, struct/enum, variabel, fungsi Rust (`src/`) & frontend (`ui/app.js`), serta event IPC.

---

## Cara Build dari Source Code

### Windows
```powershell
cargo run
# Build release teroptimasi:
cargo build --release
```

### Linux (Ubuntu / Debian)
```bash
sudo apt-get update
sudo apt-get install -y rustc cargo build-essential pkg-config libssl-dev libdbus-1-dev libgtk-3-dev libwebkit2gtk-4.1-dev rpm
chmod +x scripts/build-linux.sh
./scripts/build-linux.sh
```

---

## Lisensi

Proyek ini dilisensikan di bawah [**GNU General Public License v3.0 (GPL-3.0)**](LICENSE). Setiap modifikasi dan distribusi ulang wajib tetap bersifat *open-source* di bawah lisensi yang sama.
