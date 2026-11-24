# 03 — Kronologi Step-by-Step Prompt Pembuatan ViBrowse (Hingga V1.1.a)

Dokumen ini mendokumentasikan seluruh tahapan prompt pengguna beserta solusi rekayasa yang diterapkan dari nol hingga rilis **ViBrowse V1.1.a** lintas platform.

---

## Kronologi Lengkap Step Prompt

### Step 1 — Inisiasi Browser Ringan Setara Chrome dengan Rust
- **Prompt**: *"aku pengen bikin browser yang ringan, resourcenya kecil, fitur dan kemampuan sama dengan google chrome, kamu buatin pake rust ya"*
- **Eksekusi**: Memilih arsitektur **Rust + Tauri v2 + WebView2** dan membangun MVP lengkap dengan 5 fitur utama: Multi-Tab + Auto Tab Sleeping, Bookmarks/History/Downloads SQLite, Ad-Blocker, Mode Incognito, dan DevTools + Chrome Keyboard Shortcuts.

### Step 2 — Standarisasi & Perapian Kode Tahap Pertama
- **Prompt**: *"rapikan codenya"*
- **Eksekusi**: Menjalankan `cargo fmt`, membersihkan seluruh peringatan `cargo clippy -- -D warnings`, dan memecah kode UI panel menjadi fungsi-fungsi modular.

### Step 3 — Perbaikan Console Popup & Layout 3-Zona Disjoint
- **Prompt**: *"kenapa ada shell yang terbuka setiap aku buka appsnya dan tidak bisa membuka web yang dituju"*
- **Eksekusi**:
  - Mengubah atribut subsystem Windows menjadi `#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]` agar tidak ada CMD hitam di mode debug maupun release.
  - Memperbaiki *Win32 Z-order overlap* dengan **Layout 3-Zona Disjoint** (`chrome`, `tab-<id>`, dan `panel`) serta mengubah command pembuat webview menjadi `async fn` untuk mencegah deadlock IPC WebView2.

### Step 4 — Build Ulang & Pemasangan Logo Resmi
- **Prompt**: *"build ulang"* -> *"gunakan logo.png untuk logo dan icon vibrowse"*
- **Eksekusi**: Mengonversi `Logo.png` menjadi multi-resolusi `icons/icon.ico`, `32x32.png`, `128x128.png`, dan memasangnya di halaman New Tab serta favicon tab.

### Step 5 — Dokumentasi Teknis Awal
- **Prompt**: *"buat folder dokumen, buat dokumentasi untuk pembuatan browser ini, termasuk step prompt, tech stack dan fitur dalam bentuk markdown dokumen buat sedetail mungkin"*
- **Eksekusi**: Menyusun dokumentasi terstruktur di dalam folder `dokumen/`.

### Step 6 — Upgrade Ad-Blocker Situs Streaming/Donghua & True Monitor Fullscreen
- **Prompt**: *"ad blocker masih kurang optimal. aku coba di situs donghua, iklan yang buka tab baru masih lolos, fullscreen layarnya belum belum bisa fullscreen, hanya penuh di jendela browser, bukan max full screen, cek kesalahannya dimana kita diskkusi dulu"* -> *"ok, untuk pop up blocker bisa dikecualikan"*
- **Eksekusi**:
  - Memasang hook COM native WebView2 `WebResourceRequested` untuk memblokir `<script>` dan `<iframe>` iklan sebelum menyentuh jaringan + memperluas daftar 150+ domain iklan/pop-under + anti-overlay klik transparan.
  - Membangun **Smart Popup Blocker** pada `on_new_window` untuk memblokir pop-under lintas domain, dilengkapi tombol toolbar `Popup` (**Buka Sekali** / **Kecualikan & Buka**) dan toggle pengecualian per situs.
  - Memasang listener `ContainsFullScreenElementChanged` & shortcut `F11` untuk **True Monitor Fullscreen**.

### Step 7 — Unified Download Manager (IDM + YouTube + Native BitTorrent)
- **Prompt**: *"tambahkan fitur internet download manager untuk semua file/platform, mirip idm, youtube downloader, torrent, dengan kemampuan yang sama dengan IDM dan utorrent"*
- **Eksekusi**:
  - Membangun `src/downloader.rs` yang menyatukan **IDM Multi-Connection (4/8/16 segmen paralel)**, **Media/Video Sniffer otomatis di toolbar**, **YouTube/1000+ situs via `yt-dlp` (dengan auto-installer 1-klik)**, dan **Native Rust BitTorrent/Magnet (`librqbit 8.1`)**, lengkap dengan halaman penuh `vibrowse://downloads`.

### Step 8 — Modernisasi UI SVG, Light/Dark Mode, About Page & Versioning V1.1.a
- **Prompt**:
  1. *"perbaiki icon uinya buat lebih modern, estetik"*
  2. *"tambahkan about vibrowse, dan buat light mode dan dark mode"*
  3. *"di about tambahkan informasi fitur, dan jadikan versioning appsnya ke V1.1.a"*
- **Eksekusi**: Mengganti seluruh emoji dengan ikon vektor SVG bergaya Lucide, menambahkan tema **Light Mode & Dark Mode** tersimpan di SQLite, membangun halaman `vibrowse://about` berisi daftar fitur lengkap, dan menaikkan versi ke **`V1.1.a` (`1.1.0-a`)**.

### Step 9 — Packaging Installer & Portable ZIP + Optimasi Ukuran Ekstrem
- **Prompt**:
  1. *"ini kalau mau dishare, cukup file single exenya yang dishare ?"*
  2. *"buatkan installer dan versi portablenya dizip, simpan di folder reselase"*
  3. *"rapikan dan optimize codenya, kalau bisa kurangi ukurang filenya agar tidak terlalu besar"*
- **Eksekusi**:
  - Memverifikasi dependency DLL via `objdump` (hanya butuh `WebView2Loader.dll` selain DLL bawaan Windows).
  - Memangkas ukuran `vibrowse.exe` dari **9.53 MB -> 6.33 MB**, `Portable.zip` dari **5.26 MB -> 2.91 MB**, dan `Setup.exe` dari **4.62 MB -> 2.45 MB** melalui penyatuan stack TLS OS (`default-tls`), penghapusan `chrono`/`uuid`/`futures-util` (diganti `src/util.rs`), kompresi `ui/logo.png` (`1.54 MB -> 14.5 KB`), penghapusan query N+1 SQLite, serta `opt-level = "z"`, `lto = "fat"`, dan `--gc-sections`.

### Step 10 — Dukungan Lintas Platform Linux (`.deb` & `.rpm`) & Dokumentasi Engine
- **Prompt**:
  1. *"yang terakhir, kalau ini kita launch untuk windows, linux (.deb dan .rpm), dan android (apk) kira-kira bisa nggak ? dan bisa tetap optimal atau jadi berat di OS tertentu ? (diskusi)"*
  2. *"ok, sekarang tambahkan deploy untuk linux pakai .deb dan .rpm. di release, pisahkan folder windows, linux, android"*
  3. *"btw, web2view edge kan pakai technya chromium ya ? kalau iya, kenapa nggak ngambil punya chromium langsung aja ? (diskkusi)"*
  4. *"berarti di linux juga pakai web2view ? masih diskusi"*
  5. *"ok, diskusi kita tadi tentang web2kit dan libwebkit serta webkitgtk, kamu masukkan ke update dokumen, dan jelaskan alasan kenapa kita pakai tech/library ini juga ya, pokoknya jelasin selengkap mungkin biar aku bisa baca detail kenapa kita pakai itu"*
- **Eksekusi**:
  - Memisahkan folder `release/windows`, `release/linux`, dan `release/android`.
  - Menambahkan dukungan lintas OS Linux (`webkit2gtk` fullscreen hooks, `xdg-open`, `yt-dlp_linux` + `chmod 755`), skrip [scripts/build-linux.sh](../scripts/build-linux.sh), dan workflow CI/CD [.github/workflows/release.yml](../.github/workflows/release.yml).
  - Memperbarui seluruh dokumen di `dokumen/` serta menambahkan dokumen khusus [05-bedah-engine-webview-dan-lintas-platform.md](./05-bedah-engine-webview-dan-lintas-platform.md).
