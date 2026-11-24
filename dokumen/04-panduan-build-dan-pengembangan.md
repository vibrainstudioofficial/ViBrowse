# 04 — Panduan Build, Packaging Rilis (Windows & Linux), & Roadmap

Dokumen ini berisi panduan praktis untuk mem-build **ViBrowse V1.1.a** di Windows maupun Linux (`.deb` & `.rpm`), struktur distribusi `release/`, serta rencana pengembangan lanjutan.

---

## 1. Struktur Folder Distribusi (`release/`)

```text
release/
├── windows/
│   ├── ViBrowse-V1.1.a-Setup.exe      # (2.45 MB) Installer Windows otomatis + Desktop/Start Menu shortcut + Uninstaller + auto-check WebView2
│   └── ViBrowse-V1.1.a-Portable.zip   # (2.91 MB) Versi Portable siap ekstrak (vibrowse.exe + WebView2Loader.dll + README.txt)
├── linux/
│   ├── ViBrowse-V1.1.a-linux-amd64.deb    # Paket instalasi Debian / Ubuntu / Mint / Pop!_OS
│   └── ViBrowse-V1.1.a-linux-x86_64.rpm   # Paket instalasi Fedora / RHEL / openSUSE / Rocky
└── android/
    └── README.md                          # Folder target rilis APK Android
```

---

## 2. Build & Packaging di Windows

### 2.1 Mode Development & Cek Kualitas Kode
```powershell
# Format & lint ketat (wajib 0 warning)
cargo fmt
cargo clippy --offline --all-targets -- -D warnings

# Jalankan mode development
cargo run --offline
```

### 2.2 Build Release Ukuran Minimal (`opt-level = "z"` + Fat LTO)
```powershell
cargo build --release --offline
```
Hasil executable utama berada di `target\release\vibrowse.exe` (~6.33 MB) beserta `target\release\WebView2Loader.dll` (157 KB).
- Apakah langsung jalan di PC Windows lain tanpa install library tambahan? **Ya!** Pada Windows 11 & Windows 10 standar tidak membutuhkan Visual C++ Redistributable, Rust, maupun SQLite eksternal. Jika diinstal di Windows modifikasi yang belum punya WebView2, `ViBrowse-V1.1.a-Setup.exe` otomatis mendeteksi dan menawari instalasi WebView2 resmi Microsoft.

---

## 3. Build & Packaging di Linux (`.deb` & `.rpm`)

### 3.1 Prasyarat Paket Build di Ubuntu / Debian (atau WSL2)
Pastikan koneksi internet Linux/WSL aktif, lalu pasang paket header WebKitGTK, GTK3, OpenSSL, dan `rpm`:
```bash
sudo apt-get update
sudo apt-get install -y \
  rustc cargo build-essential pkg-config \
  libssl-dev libdbus-1-dev libgtk-3-dev libwebkit2gtk-4.1-dev \
  rpm rsync
```

### 3.2 Menjalankan Builder Otomatis `.deb` & `.rpm`
```bash
chmod +x scripts/build-linux.sh
./scripts/build-linux.sh
```
Skrip [scripts/build-linux.sh](../scripts/build-linux.sh) akan otomatis:
1. Mengompilasi binary release Linux x86_64 yang di-strip penuh.
2. Membuat desktop launcher `/usr/share/applications/vibrowse.desktop` (termasuk asosiasi klik protokol `http`, `https`, dan `magnet:`).
3. Membangun **`release/linux/ViBrowse-V1.1.a-linux-amd64.deb`** (`dpkg-deb -Zxz`) dengan dependency `libwebkit2gtk-4.1-0, libgtk-3-0`.
4. Membangun **`release/linux/ViBrowse-V1.1.a-linux-x86_64.rpm`** (`rpmbuild -bb`) dengan dependency `webkit2gtk4.1, gtk3`.

### 3.3 Cara Install di Linux Penerima
```bash
# Ubuntu / Debian / Linux Mint:
sudo apt install ./ViBrowse-V1.1.a-linux-amd64.deb

# Fedora / RHEL / Rocky:
sudo dnf install ./ViBrowse-V1.1.a-linux-x86_64.rpm
```

### 3.4 Build Otomatis via GitHub Actions CI/CD
File [.github/workflows/release.yml](../.github/workflows/release.yml) sudah dikonfigurasi untuk mem-build artefak **Windows** dan **Linux (`.deb` & `.rpm`)** secara otomatis di server GitHub setiap kali kamu melakukan push tag versi (misal `git tag V1.1.a && git push --tags`) atau klik *Run workflow*.

---

## 4. Roadmap Pengembangan Berikutnya (V1.2+)

1. **Modul Android (`.apk`)**: Workspace bersama `vibrowse-core` + plugin Kotlin `android.webkit.WebView` multi-tab + UI mobile responsif + Foreground Service unduhan.
2. **Find in Page (`Ctrl+F`)**: Pencarian & sorot kata di dalam halaman aktif.
3. **Session Restore Hemat RAM**: Memulihkan daftar tab sesi terakhir saat browser dibuka ulang dalam keadaan *Sleeping* agar startup tetap instan.
4. **Custom EasyList / Filter List Import**: Memuat aturan filter iklan eksternal format `.txt` ke dalam `AdBlocker`.
5. **Split View (2 Tab Berdampingan)**: Memanfaatkan sistem layout disjoint untuk membuka dua situs secara berdampingan kiri-kanan.
