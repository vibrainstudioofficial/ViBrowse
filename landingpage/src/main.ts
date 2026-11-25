import './style.css';
import {
  DEFAULT_VERSION,
  DOWNLOAD_PACKAGES,
  FEATURES,
  SPECS,
  type DownloadPackage,
  type PackageId,
} from './data';
import {
  animateCounter,
  fetchLiveStats,
  recordLocalDownload,
  type DownloadStatsSnapshot,
} from './stats';

const SUN_ICON =
  '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/></svg>';
const MOON_ICON =
  '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/></svg>';

let latestSnapshot: DownloadStatsSnapshot | null = null;

function $(id: string): HTMLElement {
  const el = document.getElementById(id);
  if (!el) throw new Error(`Element #${id} tidak ditemukan`);
  return el;
}

function detectUserOs(): 'windows' | 'linux' {
  const ua = navigator.userAgent.toLowerCase();
  if (ua.includes('linux') && !ua.includes('android')) return 'linux';
  return 'windows';
}

function initTheme(): void {
  const saved = localStorage.getItem('vibrowse_landing_theme');
  const initial = saved === 'light' || saved === 'dark' ? saved : 'dark';
  applyTheme(initial);

  $('theme-toggle').addEventListener('click', () => {
    const next = document.documentElement.dataset.theme === 'dark' ? 'light' : 'dark';
    applyTheme(next);
    localStorage.setItem('vibrowse_landing_theme', next);
  });
}

function applyTheme(theme: 'dark' | 'light'): void {
  document.documentElement.dataset.theme = theme;
  $('theme-toggle').innerHTML = theme === 'dark' ? SUN_ICON : MOON_ICON;
}

function setupHeroOsDetection(): void {
  const os = detectUserOs();
  const primaryPkg: DownloadPackage =
    os === 'linux' ? DOWNLOAD_PACKAGES[2] : DOWNLOAD_PACKAGES[0];
  const btn = $('primary-dl-btn') as HTMLAnchorElement;
  btn.href = primaryPkg.url;
  btn.dataset.pkgId = primaryPkg.id;
  $('primary-dl-label').textContent = `Unduh untuk ${os === 'linux' ? 'Linux (.deb)' : 'Windows'} (${primaryPkg.size})`;
  $('os-detect-note').textContent = `Terdeteksi: ${
    os === 'linux' ? 'Linux (x64)' : 'Windows 10/11 (x64)'
  } • ${primaryPkg.filename}`;

  btn.addEventListener('click', () => handleDownloadClick(primaryPkg.id));
}

function renderStaticSections(): void {
  const featureGrid = $('feature-grid');
  featureGrid.innerHTML = FEATURES.map(
    (f) => `
      <article class="card feature-card">
        <span class="tag">${f.tag}</span>
        <h3>${f.title}</h3>
        <p class="muted">${f.desc}</p>
      </article>
    `
  ).join('');

  const specList = $('spec-list');
  specList.innerHTML = SPECS.map(
    (s) => `
      <div class="spec-row">
        <span class="muted">${s.label}</span>
        <strong>${s.value}</strong>
      </div>
    `
  ).join('');
}

function renderDownloadCards(snapshot: DownloadStatsSnapshot | null): void {
  const grid = $('download-grid');
  grid.innerHTML = DOWNLOAD_PACKAGES.map((pkg) => {
    const url = snapshot?.liveUrls[pkg.id] ?? pkg.url;
    const count = snapshot?.perPackage[pkg.id] ?? 0;
    return `
      <article class="card dl-card ${pkg.recommended ? 'dl-card-featured' : ''}">
        <div class="dl-head">
          <div>
            <h3>${pkg.title}</h3>
            <p class="muted small">${pkg.subtitle}</p>
          </div>
          <span class="tag ${pkg.recommended ? 'tag-accent' : ''}">${pkg.badge}</span>
        </div>
        <div class="dl-meta">
          <code>${pkg.filename}</code>
          <span>${pkg.size}</span>
        </div>
        <p class="muted small dl-hint">${pkg.hint}</p>
        <div class="dl-foot">
          <span class="muted small">${count.toLocaleString('id-ID')} unduhan</span>
          <a
            href="${url}"
            class="btn ${pkg.recommended ? 'btn-primary' : 'btn-ghost'} btn-sm"
            data-dl-pkg="${pkg.id}"
            rel="noopener noreferrer"
          >
            Unduh
          </a>
        </div>
      </article>
    `;
  }).join('');

  grid.querySelectorAll<HTMLAnchorElement>('[data-dl-pkg]').forEach((link) => {
    link.addEventListener('click', () => {
      const pkgId = link.dataset.dlPkg as PackageId | undefined;
      if (pkgId) handleDownloadClick(pkgId);
    });
  });
}

function renderPlatformBars(snapshot: DownloadStatsSnapshot): void {
  const barsEl = $('platform-bars');
  const maxVal = Math.max(1, ...DOWNLOAD_PACKAGES.map((p) => snapshot.perPackage[p.id] ?? 0));
  const total = Math.max(1, snapshot.totalDownloads);

  barsEl.innerHTML = DOWNLOAD_PACKAGES.map((pkg) => {
    const count = snapshot.perPackage[pkg.id] ?? 0;
    const pctTotal = Math.round((count / total) * 100);
    const widthPct = count === 0 ? 2 : Math.max(6, Math.round((count / maxVal) * 100));
    return `
      <div class="bar-item">
        <div class="bar-label-row">
          <span><strong>${pkg.title}</strong> <span class="muted small">(${pkg.size})</span></span>
          <span class="small"><strong>${count.toLocaleString('id-ID')}</strong> <span class="muted">(${pctTotal}%)</span></span>
        </div>
        <div class="bar-track">
          <div class="bar-fill" style="width: ${widthPct}%"></div>
        </div>
      </div>
    `;
  }).join('');
}

function applySnapshot(snapshot: DownloadStatsSnapshot): void {
  latestSnapshot = snapshot;

  $('nav-version').textContent = snapshot.latestVersion;
  $('stat-latest-version').textContent = snapshot.latestVersion;
  $('hero-release-tag').textContent = `Rilis Terbaru ${snapshot.latestVersion} • Open Source GPL-3.0`;

  if (snapshot.publishedAt) {
    const dateStr = new Date(snapshot.publishedAt).toLocaleDateString('id-ID', {
      day: 'numeric',
      month: 'short',
      year: 'numeric',
    });
    $('stat-release-date').textContent = `Dirilis ${dateStr}`;
  } else {
    $('stat-release-date').textContent = `Rilis ${DEFAULT_VERSION} • Windows & Linux`;
  }

  animateCounter($('stat-total-downloads'), snapshot.totalDownloads);
  animateCounter($('stat-session-downloads'), snapshot.sessionDownloads);

  const liveDot = $('live-dot');
  const liveText = $('live-status-text');
  if (snapshot.source === 'github-live') {
    liveDot.classList.remove('offline');
    liveText.textContent = 'Live GitHub API';
    $('stat-total-sub').textContent = 'Data real-time dari GitHub Releases';
  } else {
    liveDot.classList.add('offline');
    liveText.textContent = 'Live Counter Lokal';
    $('stat-total-sub').textContent = 'Menghitung lokal (GitHub API belum publik)';
  }

  const timeStr = snapshot.fetchedAt.toLocaleTimeString('id-ID', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
  $('stat-last-updated').textContent = `Terakhir sinkron: ${timeStr} (auto-refresh 60 dtk)`;

  // Keep primary hero CTA synced with live download URL if available
  const primaryBtn = $('primary-dl-btn') as HTMLAnchorElement;
  const primaryPkgId = primaryBtn.dataset.pkgId as PackageId | undefined;
  if (primaryPkgId && snapshot.liveUrls[primaryPkgId]) {
    primaryBtn.href = snapshot.liveUrls[primaryPkgId]!;
  }

  renderPlatformBars(snapshot);
  renderDownloadCards(snapshot);
}

function handleDownloadClick(pkgId: PackageId): void {
  recordLocalDownload(pkgId);
  if (latestSnapshot) {
    latestSnapshot = {
      ...latestSnapshot,
      totalDownloads: latestSnapshot.totalDownloads + 1,
      sessionDownloads: latestSnapshot.sessionDownloads + 1,
      perPackage: {
        ...latestSnapshot.perPackage,
        [pkgId]: (latestSnapshot.perPackage[pkgId] ?? 0) + 1,
      },
      fetchedAt: new Date(),
    };
    applySnapshot(latestSnapshot);
  }
}

async function refreshStats(): Promise<void> {
  const btn = $('btn-refresh-stats') as HTMLButtonElement;
  btn.disabled = true;
  btn.textContent = '...';
  try {
    const snap = await fetchLiveStats();
    applySnapshot(snap);
  } finally {
    btn.disabled = false;
    btn.textContent = 'Refresh';
  }
}

async function bootstrap(): Promise<void> {
  initTheme();
  setupHeroOsDetection();
  renderStaticSections();
  renderDownloadCards(null);

  $('btn-refresh-stats').addEventListener('click', () => {
    void refreshStats();
  });

  await refreshStats();
  window.setInterval(() => {
    void refreshStats();
  }, 60_000);
}

void bootstrap();
