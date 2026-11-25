export type PackageId = 'win-setup' | 'win-portable' | 'linux-deb' | 'linux-rpm';

export interface DownloadPackage {
  id: PackageId;
  os: 'windows' | 'linux';
  title: string;
  subtitle: string;
  filename: string;
  size: string;
  recommended?: boolean;
  badge: string;
  url: string;
  hint: string;
}

export interface FeatureItem {
  title: string;
  desc: string;
  tag: string;
}

export interface SpecItem {
  label: string;
  value: string;
}

export const GITHUB_REPO = 'fransetiaone/ViBrowse';
export const DEFAULT_VERSION = 'V1.2a';
export const RELEASE_BASE_URL = `https://github.com/${GITHUB_REPO}/releases/download/${DEFAULT_VERSION}`;

export const DOWNLOAD_PACKAGES: DownloadPackage[] = [
  {
    id: 'win-setup',
    os: 'windows',
    title: 'Windows Installer',
    subtitle: 'Windows 10 / 11 (x64)',
    filename: `ViBrowse-${DEFAULT_VERSION}-Setup.exe`,
    size: '2.46 MB',
    recommended: true,
    badge: 'Direkomendasikan',
    url: `${RELEASE_BASE_URL}/ViBrowse-${DEFAULT_VERSION}-Setup.exe`,
    hint: 'Otomatis membuat shortcut Desktop, Start Menu, registrasi Default Browser, & Uninstaller tanpa butuh Admin.',
  },
  {
    id: 'win-portable',
    os: 'windows',
    title: 'Windows Portable',
    subtitle: 'Windows 10 / 11 (x64)',
    filename: `ViBrowse-${DEFAULT_VERSION}-Portable.zip`,
    size: '2.93 MB',
    badge: 'Tanpa Instalasi',
    url: `${RELEASE_BASE_URL}/ViBrowse-${DEFAULT_VERSION}-Portable.zip`,
    hint: 'Ekstrak arsip ZIP lalu jalankan langsung vibrowse.exe dari folder mana saja.',
  },
  {
    id: 'linux-deb',
    os: 'linux',
    title: 'Linux Debian / Ubuntu',
    subtitle: 'Ubuntu, Debian, Mint, Pop!_OS (x64)',
    filename: `ViBrowse-${DEFAULT_VERSION}-linux-amd64.deb`,
    size: '~3–5 MB',
    badge: '.DEB Package',
    url: `${RELEASE_BASE_URL}/ViBrowse-${DEFAULT_VERSION}-linux-amd64.deb`,
    hint: 'Pasang via terminal: sudo apt install ./ViBrowse-*.deb',
  },
  {
    id: 'linux-rpm',
    os: 'linux',
    title: 'Linux Fedora / RHEL',
    subtitle: 'Fedora, openSUSE, Rocky, RHEL (x64)',
    filename: `ViBrowse-${DEFAULT_VERSION}-linux-x86_64.rpm`,
    size: '~3–5 MB',
    badge: '.RPM Package',
    url: `${RELEASE_BASE_URL}/ViBrowse-${DEFAULT_VERSION}-linux-x86_64.rpm`,
    hint: 'Pasang via terminal: sudo dnf install ./ViBrowse-*.rpm',
  },
];

export const FEATURES: FeatureItem[] = [
  {
    tag: '2.46 MB',
    title: 'Ukuran Ekstrem Ringan',
    desc: 'Dikompilasi dengan Rust + Fat LTO tanpa membundel Chromium 200 MB. Unduh dan pasang dalam hitungan detik.',
  },
  {
    tag: 'RAM Saver',
    title: 'Auto Tab Sleeping',
    desc: 'Worker latar belakang Tokio menidurkan tab yang tidak aktif agar RAM bebas 100% tanpa kehilangan URL maupun posisi tab.',
  },
  {
    tag: 'Proteksi',
    title: 'Native Ad & Popup Blocker',
    desc: 'Mencegat 150+ domain iklan & tracker di level engine WebView sebelum menyentuh jaringan, lengkap dengan pengecualian per situs.',
  },
  {
    tag: 'IDM + P2P',
    title: 'Download Manager Terpadu',
    desc: 'Akselerator HTTP hingga 16 koneksi paralel, tombol floating unduh YouTube/X/Video ala IDM, serta klien BitTorrent/Magnet native Rust.',
  },
  {
    tag: 'Otomatis',
    title: 'Auto Update & Default Browser',
    desc: 'Deteksi pembaruan otomatis dari GitHub Releases (1-klik pasang update) serta banner pengecekan Default Browser ala Chrome.',
  },
  {
    tag: 'Modern UI',
    title: 'Privasi, Light/Dark & Fullscreen',
    desc: 'Mode Incognito + SQLite lokal tanpa akun, tema terang/gelap instan, Chrome shortcuts, DevTools (F12), dan True Monitor Fullscreen (F11).',
  },
];

export const SPECS: SpecItem[] = [
  { label: 'Core Backend', value: 'Rust + Tauri v2 (Async Tokio)' },
  { label: 'Engine Windows', value: 'Microsoft Edge WebView2 (Blink + V8)' },
  { label: 'Engine Linux', value: 'WebKitGTK (libwebkit2gtk-4.1)' },
  { label: 'Penyimpanan Lokal', value: 'SQLite WAL (Bundled)' },
  { label: 'BitTorrent Engine', value: 'librqbit (Native Rust P2P/DHT)' },
  { label: 'Lisensi', value: 'GNU GPL-3.0 (Open Source)' },
];
