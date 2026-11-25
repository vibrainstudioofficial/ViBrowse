const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const NEWTAB_URL = 'vibrowse://newtab';
const DOWNLOADS_URL = 'vibrowse://downloads';
const ABOUT_URL = 'vibrowse://about';
const IS_PANEL_VIEW = new URLSearchParams(window.location.search).get('view') === 'panel';

if (IS_PANEL_VIEW) document.body.classList.add('panel-only');

let state = { tabs: [], active_tab_id: null, active_panel: null, settings: {} };
let dmTasks = [];
let dmEngineStatus = { ytdlp_installed: false };
let dmAddMode = 'http';
let dmFilter = 'all';
let bookmarksCache = [];

const $ = (id) => document.getElementById(id);

const ICONS = {
  search: '<circle cx="11" cy="11" r="7"/><path d="M21 21l-4.35-4.35"/>',
  lock: '<rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/>',
  warn: '<path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/>',
  incognito: '<path d="M2 12h20"/><path d="M6 12l2-6h8l2 6"/><circle cx="8" cy="17" r="3"/><circle cx="16" cy="17" r="3"/><path d="M11 17h2"/>',
  star: '<polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>',
  bookmark: '<path d="M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z"/>',
  history: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
  download: '<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/>',
  shield: '<path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>',
  settings: '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9c.26.6.85 1 1.51 1H21a2 2 0 1 1 0 4h-.09c-.66 0-1.25.4-1.51 1z"/>',
  sleep: '<path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/>',
  pin: '<path d="M12 17v5"/><path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16h14v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z"/>',
  close: '<line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>',
  trash: '<polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M10 11v6M14 11v6"/><path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/>',
  file: '<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/>',
  folder: '<path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>',
  pause: '<rect x="6" y="4" width="4" height="16" rx="1"/><rect x="14" y="4" width="4" height="16" rx="1"/>',
  play: '<polygon points="6 4 20 12 6 20 6 4"/>',
  zap: '<path d="M13 2L3 14h9l-1 8 10-12h-9l1-8z"/>',
  video: '<rect x="2" y="4" width="20" height="16" rx="2"/><polygon points="10 9 15 12 10 15 10 9"/>',
  magnet: '<path d="M6 15V7a6 6 0 1 1 12 0v8"/><path d="M6 15h4v4H6z"/><path d="M14 15h4v4h-4z"/>',
  ban: '<circle cx="12" cy="12" r="9"/><path d="M5.6 5.6l12.8 12.8"/>',
  sun: '<circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/>',
  info: '<circle cx="12" cy="12" r="9"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/>',
};

// ---- HELPERS ----

function iconSvg(name, sizeClass = 'ui-icon-sm', extraClass = '') {
  return `<svg class="ui-icon ${sizeClass} ${extraClass}" viewBox="0 0 24 24">${ICONS[name] || ''}</svg>`;
}

function createBadge(iconName, sizeClass = 'ui-icon-sm', extraClass = '') {
  const el = document.createElement('span');
  el.className = `tab-badge ${extraClass}`.trim();
  el.innerHTML = iconSvg(iconName, sizeClass);
  return el;
}

function isInternal(url) {
  return typeof url === 'string' && url.startsWith('vibrowse://');
}

function activeTab() {
  return state.tabs.find((tab) => tab.id === state.active_tab_id) || null;
}

function withTab(fn) {
  const tab = activeTab();
  if (tab) fn(tab);
}

function navigateActive(input) {
  withTab((tab) => invoke('cmd_navigate', { tabId: tab.id, input }));
}

function saveSettings(patch) {
  return invoke('cmd_update_settings', { settings: { ...state.settings, ...patch } });
}

function escapeHtml(str) {
  return String(str || '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function hostOf(url) {
  try {
    return new URL(url).hostname;
  } catch {
    return url;
  }
}

function fmtBytes(bytes) {
  if (!bytes || bytes <= 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const idx = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / Math.pow(1024, idx)).toFixed(idx === 0 ? 0 : 1)} ${units[idx]}`;
}

function fmtEta(secs) {
  if (!secs || secs <= 0) return '--';
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  if (h > 0) return `${h}j ${m}m`;
  if (m > 0) return `${m}m ${s}d`;
  return `${s}d`;
}

// ---- CHROME RENDERING ----

function renderTabs() {
  const container = $('tabs-container');
  container.innerHTML = '';

  for (const tab of state.tabs) {
    const isActive = tab.id === state.active_tab_id;
    const el = document.createElement('div');
    el.className = `tab${isActive ? ' active' : ''}${tab.is_incognito ? ' incognito' : ''}${tab.is_sleeping ? ' sleeping' : ''}`;
    el.title = `${tab.title}\n${tab.url}${tab.is_sleeping ? '\n(Sleeping — click to wake)' : ''}`;

    if (tab.is_loading) {
      const sp = document.createElement('span');
      sp.className = 'tab-spinner';
      el.appendChild(sp);
    } else if (tab.is_incognito) {
      el.appendChild(createBadge('incognito', 'ui-icon-sm', 'incognito-icon'));
    } else if (tab.favicon || tab.url === NEWTAB_URL) {
      const img = document.createElement('img');
      img.className = 'tab-favicon';
      img.src = tab.favicon || 'logo.png';
      img.onerror = () => { img.style.display = 'none'; };
      el.appendChild(img);
    } else if (tab.url === DOWNLOADS_URL) {
      el.appendChild(createBadge('zap'));
    } else if (tab.url === ABOUT_URL) {
      el.appendChild(createBadge('info'));
    }

    if (tab.pinned) el.appendChild(createBadge('pin', 'ui-icon-xs', 'pin-icon'));
    if (tab.is_sleeping) el.appendChild(createBadge('sleep', 'ui-icon-xs', 'sleep-icon'));

    const titleEl = document.createElement('span');
    titleEl.className = 'tab-title';
    titleEl.textContent = tab.title || 'New Tab';
    el.appendChild(titleEl);

    if (!isActive && !tab.is_sleeping && !isInternal(tab.url)) {
      const sleepBtn = document.createElement('button');
      sleepBtn.className = 'tab-btn';
      sleepBtn.title = 'Sleep tab to free RAM';
      sleepBtn.innerHTML = iconSvg('sleep', 'ui-icon-xs');
      sleepBtn.onclick = (e) => { e.stopPropagation(); invoke('cmd_sleep_tab', { tabId: tab.id }); };
      el.appendChild(sleepBtn);
    }

    const closeBtn = document.createElement('button');
    closeBtn.className = 'tab-btn';
    closeBtn.title = 'Close tab (Ctrl+W)';
    closeBtn.innerHTML = iconSvg('close', 'ui-icon-xs');
    closeBtn.onclick = (e) => { e.stopPropagation(); invoke('cmd_close_tab', { tabId: tab.id }); };
    el.appendChild(closeBtn);

    el.onclick = () => invoke('cmd_switch_tab', { tabId: tab.id });
    el.onauxclick = (e) => { if (e.button === 1) invoke('cmd_close_tab', { tabId: tab.id }); };
    el.oncontextmenu = (e) => { e.preventDefault(); invoke('cmd_toggle_pin_tab', { tabId: tab.id }); };
    container.appendChild(el);
  }
}

function renderToolbar() {
  const tab = activeTab();
  const omnibox = $('omnibox');
  const secBadge = $('security-badge');
  const bmBtn = $('btn-bookmark');

  $('nav-toolbar').classList.toggle('incognito-mode', Boolean(tab && tab.is_incognito));
  if (document.activeElement !== omnibox) {
    omnibox.value = !tab || tab.url === NEWTAB_URL ? '' : tab.url;
  }

  secBadge.className = 'security-badge';
  if (tab && tab.is_incognito) {
    secBadge.classList.add('incognito');
    secBadge.innerHTML = iconSvg('incognito');
    secBadge.title = 'Incognito Mode — History is not saved';
  } else if (tab && tab.url.startsWith('https://')) {
    secBadge.classList.add('secure');
    secBadge.innerHTML = iconSvg('lock');
    secBadge.title = 'Secure HTTPS connection';
  } else if (tab && tab.url.startsWith('http://')) {
    secBadge.classList.add('warn');
    secBadge.innerHTML = iconSvg('warn');
    secBadge.title = 'Not secure (HTTP)';
  } else {
    secBadge.innerHTML = iconSvg('search');
    secBadge.title = 'Search or enter URL';
  }

  const isBm = Boolean(tab && tab.is_bookmarked);
  bmBtn.innerHTML = iconSvg('star', 'ui-icon-sm', isBm ? 'filled' : '');
  bmBtn.classList.toggle('bookmarked', isBm);

  $('adblock-count').textContent = state.adblock_blocked_count || 0;

  const mediaCount = tab && tab.detected_media ? tab.detected_media.length : 0;
  $('btn-media-sniffer').classList.toggle('hidden', mediaCount === 0);
  if (mediaCount > 0) $('media-count').textContent = `${mediaCount} Media`;

  const popupBtn = $('btn-popup-blocked');
  popupBtn.classList.toggle('hidden', !(tab && tab.last_blocked_popup));
  if (tab && tab.last_blocked_popup) {
    popupBtn.title = `Popup diblokir: ${tab.last_blocked_popup} — Klik untuk kelola/buka`;
  }

  document.querySelectorAll('[data-panel-btn]').forEach((btn) => {
    btn.classList.toggle('active-btn', state.active_panel === btn.dataset.panelBtn);
  });

  const isLight = state.settings.theme === 'light';
  const themeBtn = $('btn-theme-toggle');
  if (themeBtn) {
    themeBtn.innerHTML = iconSvg(isLight ? 'sleep' : 'sun', 'ui-icon');
    themeBtn.title = isLight ? 'Switch to Dark Mode' : 'Switch to Light Mode';
  }

  $('ram-pill-text').textContent = state.sleeping_tabs_count > 0
    ? `${state.sleeping_tabs_count} Sleeping • ~${state.estimated_ram_saved_mb}MB saved`
    : `RAM Saver (${state.active_webviews_count || 0} active)`;
}

function renderBookmarksBar() {
  const bar = $('bookmarks-bar');
  bar.classList.toggle('hidden', !state.settings.show_bookmarks_bar);
  if (!state.settings.show_bookmarks_bar) return;

  bar.innerHTML = '';
  for (const bm of bookmarksCache) {
    const el = document.createElement('button');
    el.className = 'bm-chip';
    el.title = bm.url;
    el.innerHTML = `${iconSvg('star', 'ui-icon-xs')}<span>${escapeHtml(bm.title)}</span>`;
    el.onclick = () => navigateActive(bm.url);
    bar.appendChild(el);
  }
}

// ---- INTERNAL PAGES ----

function renderNewTabPage() {
  const tab = activeTab();
  const isIncognito = Boolean(tab && tab.is_incognito);

  $('newtab-page').classList.toggle('incognito-ntp', isIncognito);
  $('ntp-heading').textContent = isIncognito ? 'ViBrowse Incognito' : 'ViBrowse';
  $('ntp-sub').textContent = isIncognito
    ? 'Private browsing — Cookies & history are not saved'
    : 'Lightweight Rust Browser • Full Chrome Compatibility';

  $('stat-active').textContent = state.active_webviews_count || 0;
  $('stat-sleeping').textContent = state.sleeping_tabs_count || 0;
  $('stat-ram').textContent = `${state.estimated_ram_saved_mb || 0} MB`;
  $('stat-blocked').textContent = state.adblock_blocked_count || 0;

  const dials = $('ntp-dials');
  dials.innerHTML = '';
  for (const bm of bookmarksCache.slice(0, 8)) {
    const el = document.createElement('div');
    el.className = 'dial-card';
    const host = hostOf(bm.url);
    el.innerHTML = `
      <div class="dial-icon">${escapeHtml((bm.title || host)[0].toUpperCase())}</div>
      <div class="dial-text">
        <div class="dial-title">${escapeHtml(bm.title)}</div>
        <div class="dial-host">${escapeHtml(host)}</div>
      </div>
    `;
    el.onclick = () => navigateActive(bm.url);
    dials.appendChild(el);
  }
}

function renderInternalPages() {
  const tab = activeTab();
  const url = tab ? tab.url : NEWTAB_URL;
  const isDm = url === DOWNLOADS_URL;
  const isAbout = url === ABOUT_URL;

  $('downloads-page').classList.toggle('hidden', !isDm);
  $('about-page').classList.toggle('hidden', !isAbout);
  $('newtab-page').classList.toggle('hidden', isDm || isAbout);

  if (isAbout) {
    $('about-theme-val').textContent = state.settings.theme === 'light' ? 'Light Mode' : 'Dark Mode';
    $('about-blocked-val').textContent = state.adblock_blocked_count || 0;
    $('about-ram-val').textContent = `${state.estimated_ram_saved_mb || 0} MB`;
  }
}

function renderDownloadsDashboard() {
  renderInternalPages();
  const tab = activeTab();
  if (!tab || tab.url !== DOWNLOADS_URL) return;

  const badges = $('dm-engine-badges');
  badges.innerHTML = `
    <span class="dm-engine-pill ok">${iconSvg('zap', 'ui-icon-xs')}<span>IDM 16-Conn: Siap</span></span>
    <span class="dm-engine-pill ok">${iconSvg('magnet', 'ui-icon-xs')}<span>BitTorrent Native: Siap</span></span>
    ${dmEngineStatus.ytdlp_installed
      ? `<span class="dm-engine-pill ok">${iconSvg('video', 'ui-icon-xs')}<span>yt-dlp (1000+ Situs): Siap</span></span>`
      : `<button id="btn-install-ytdlp" class="dm-engine-pill warn">${iconSvg('download', 'ui-icon-xs')}<span>Install yt-dlp (1-Klik)</span></button>`}
  `;

  const installBtn = $('btn-install-ytdlp');
  if (installBtn) {
    installBtn.onclick = async () => {
      installBtn.textContent = 'Mengunduh yt-dlp...';
      installBtn.disabled = true;
      try {
        await invoke('cmd_dm_install_ytdlp');
        dmEngineStatus = await invoke('cmd_dm_engine_status');
        renderDownloadsDashboard();
      } catch (err) {
        installBtn.textContent = 'Gagal, Coba Lagi';
        installBtn.disabled = false;
        alert(String(err));
      }
    };
  }

  const listEl = $('dm-task-list');
  const filtered = dmTasks.filter((task) => {
    if (dmFilter === 'all') return true;
    if (dmFilter === 'active') return ['downloading', 'queued', 'paused'].includes(task.status);
    if (dmFilter === 'completed') return task.status === 'completed';
    return task.kind === dmFilter;
  });

  if (filtered.length === 0) {
    listEl.innerHTML = '<p class="muted">Belum ada tugas unduhan pada kategori ini. Tempel URL file, link YouTube/TikTok, atau Magnet link di atas!</p>';
    return;
  }

  listEl.innerHTML = '';
  for (const task of filtered) {
    const pct = task.total_bytes > 0
      ? Math.min(100, (task.downloaded_bytes / task.total_bytes) * 100)
      : (task.status === 'completed' ? 100 : 0);
    const kindIcon = task.kind === 'torrent' ? 'magnet' : (task.kind === 'video' ? 'video' : 'zap');
    const fillClass = task.status === 'completed' ? 'done' : (task.status === 'failed' ? 'err' : '');

    const el = document.createElement('div');
    el.className = 'dm-task-card';
    el.innerHTML = `
      <div class="dm-task-head">
        <div class="dm-task-title-wrap">
          ${iconSvg(kindIcon)}
          <span class="dm-task-name" title="${escapeHtml(task.url)}">${escapeHtml(task.filename)}</span>
          <span class="dm-tag">${escapeHtml(task.quality)}</span>
        </div>
        <div class="dm-task-actions">
          ${task.status === 'downloading' ? `<button class="icon-btn" data-a="pause" title="Pause">${iconSvg('pause')}</button>` : ''}
          ${task.status === 'paused' ? `<button class="icon-btn" data-a="resume" title="Resume">${iconSvg('play')}</button>` : ''}
          ${['downloading', 'paused', 'queued'].includes(task.status) ? `<button class="icon-btn" data-a="cancel" title="Cancel">${iconSvg('close')}</button>` : ''}
          ${task.status === 'completed' ? `<button class="icon-btn" data-a="open" title="Buka File">${iconSvg('file')}</button>` : ''}
          <button class="icon-btn" data-a="folder" title="Buka Folder">${iconSvg('folder')}</button>
          <button class="icon-btn" data-a="remove" title="Hapus dari Daftar">${iconSvg('trash')}</button>
        </div>
      </div>
      <div class="dm-progress-track">
        <div class="dm-progress-fill ${fillClass}" style="width:${pct.toFixed(1)}%"></div>
      </div>
      <div class="dm-task-meta">
        <span>${task.status.toUpperCase()} • ${pct.toFixed(1)}% (${fmtBytes(task.downloaded_bytes)} / ${task.total_bytes > 0 ? fmtBytes(task.total_bytes) : '?'})</span>
        <span>
          ↓ ${fmtBytes(task.speed_bps)}/s
          ${task.kind === 'torrent' ? ` • ↑ ${fmtBytes(task.upload_speed_bps)}/s • ${task.peers} peers` : ''}
          • ETA: ${fmtEta(task.eta_secs)}
        </span>
      </div>
      ${task.error ? `<div class="dm-error">${escapeHtml(task.error)}</div>` : ''}
    `;

    const bind = (sel, fn) => { const b = el.querySelector(sel); if (b) b.onclick = fn; };
    bind('[data-a="pause"]', () => invoke('cmd_dm_pause', { id: task.id }));
    bind('[data-a="resume"]', () => invoke('cmd_dm_resume', { id: task.id }));
    bind('[data-a="cancel"]', () => invoke('cmd_dm_cancel', { id: task.id }));
    bind('[data-a="open"]', () => invoke('cmd_open_path', { path: task.save_path, revealInFolder: false }));
    bind('[data-a="folder"]', () => invoke('cmd_open_path', { path: task.save_path, revealInFolder: true }));
    bind('[data-a="remove"]', () => invoke('cmd_dm_remove', { id: task.id }));
    listEl.appendChild(el);
  }
}

// ---- SIDE PANELS ----

async function renderBookmarksPanel(body) {
  bookmarksCache = await invoke('cmd_list_bookmarks');
  body.innerHTML = bookmarksCache.length === 0
    ? '<p class="muted">No bookmarks yet. Click the star in the address bar to save a page.</p>'
    : '';

  for (const bm of bookmarksCache) {
    const el = document.createElement('div');
    el.className = 'panel-list-item';
    el.innerHTML = `
      <div class="panel-item-main">
        <div class="panel-item-title">${escapeHtml(bm.title)}</div>
        <div class="panel-item-sub">${escapeHtml(bm.url)}</div>
      </div>
      <button class="icon-btn" title="Remove">${iconSvg('trash')}</button>
    `;
    el.querySelector('.panel-item-main').onclick = () => navigateActive(bm.url);
    el.querySelector('button').onclick = async () => {
      await invoke('cmd_remove_bookmark', { url: bm.url });
      await refreshBookmarks();
      renderSidePanel();
    };
    body.appendChild(el);
  }
}

async function renderHistoryPanel(body) {
  body.innerHTML = `
    <div class="panel-row">
      <input id="hist-search" class="panel-search" placeholder="Search history..." />
      <button id="btn-clear-hist" class="btn btn-danger">Clear</button>
    </div>
    <div id="hist-list" class="panel-stack"></div>
  `;

  const loadHist = async (query = '') => {
    const entries = await invoke('cmd_list_history', { query });
    const listEl = $('hist-list');
    listEl.innerHTML = entries.length === 0 ? '<p class="muted">No history entries.</p>' : '';

    for (const entry of entries) {
      const el = document.createElement('div');
      el.className = 'panel-list-item';
      el.innerHTML = `
        <div class="panel-item-main">
          <div class="panel-item-title">${escapeHtml(entry.title)}</div>
          <div class="panel-item-sub">${escapeHtml(entry.url)} • ${entry.visit_count}x</div>
        </div>
        <button class="icon-btn" title="Delete">${iconSvg('close')}</button>
      `;
      el.querySelector('.panel-item-main').onclick = () => navigateActive(entry.url);
      el.querySelector('button').onclick = async () => {
        await invoke('cmd_delete_history_entry', { id: entry.id });
        loadHist($('hist-search').value);
      };
      listEl.appendChild(el);
    }
  };

  $('hist-search').oninput = (e) => loadHist(e.target.value);
  $('btn-clear-hist').onclick = async () => { await invoke('cmd_clear_history'); loadHist(''); };
  await loadHist('');
}

async function renderDownloadsPanel(body) {
  const tab = activeTab();
  const mediaList = tab && tab.detected_media ? tab.detected_media : [];
  dmTasks = await invoke('cmd_dm_list');

  body.innerHTML = `
    <div class="panel-row">
      <button id="btn-open-dm-full" class="btn btn-primary btn-with-icon" style="flex:1;">${iconSvg('zap')}<span>Buka Dashboard IDM / Torrent</span></button>
      <button id="btn-clear-dl" class="btn btn-danger">Clear</button>
    </div>
    ${mediaList.length > 0 ? `
      <div class="setting-group">
        <strong class="btn-with-icon">${iconSvg('video')}<span>Video / Media Terdeteksi (${mediaList.length})</span></strong>
        <div id="sniffer-list" class="panel-stack"></div>
      </div>` : ''}
    <div id="dl-list" class="panel-stack"></div>
  `;

  $('btn-open-dm-full').onclick = () => {
    invoke('cmd_set_panel', { panel: null });
    invoke('cmd_new_tab', { url: DOWNLOADS_URL, incognito: false });
  };
  $('btn-clear-dl').onclick = async () => {
    await invoke('cmd_dm_clear_finished');
    renderSidePanel();
  };

  if (mediaList.length > 0) {
    const snifferEl = $('sniffer-list');
    for (const media of mediaList) {
      const el = document.createElement('div');
      el.className = 'panel-list-item';
      const isStream = media.kind === 'platform' || media.kind === 'hls';
      el.innerHTML = `
        <div class="panel-item-main">
          <div class="panel-item-title btn-with-icon">${iconSvg('video', 'ui-icon-xs')}<span>${escapeHtml(media.filename)}</span></div>
          <div class="panel-item-sub">${escapeHtml(media.kind.toUpperCase())} • ${escapeHtml(media.url)}</div>
        </div>
        ${isStream ? `
          <select class="setting-select" style="font-size:11px;padding:4px 6px;" title="Pilih kualitas">
            <option value="best">Best</option>
            <option value="1080p">1080p</option>
            <option value="720p">720p</option>
            <option value="480p">480p</option>
            <option value="audio">Audio</option>
          </select>` : ''}
        <button class="btn btn-primary btn-with-icon" title="Download">${iconSvg('download', 'ui-icon-xs')}</button>
      `;
      el.querySelector('button').onclick = async () => {
        try {
          if (isStream) {
            const qSel = el.querySelector('select');
            await invoke('cmd_dm_start_video', { url: media.url, quality: qSel ? qSel.value : 'best' });
          } else {
            await invoke('cmd_dm_start_http', { url: media.url, connections: 8, filename: media.filename });
          }
          renderSidePanel();
        } catch (err) {
          alert(String(err));
        }
      };
      snifferEl.appendChild(el);
    }
  }

  const listEl = $('dl-list');
  if (dmTasks.length === 0) {
    listEl.innerHTML = '<p class="muted">Belum ada unduhan. Klik tombol di atas untuk membuka Dashboard IDM / YouTube / Torrent.</p>';
    return;
  }

  for (const task of dmTasks) {
    const pct = task.total_bytes > 0 ? Math.min(100, Math.round((task.downloaded_bytes / task.total_bytes) * 100)) : 0;
    const el = document.createElement('div');
    el.className = 'panel-list-item';
    el.innerHTML = `
      <div class="panel-item-main">
        <div class="panel-item-title">${escapeHtml(task.filename)}</div>
        <div class="panel-item-sub">${task.status.toUpperCase()} • ${pct}% • ${fmtBytes(task.speed_bps)}/s</div>
      </div>
      <button class="icon-btn" data-act="folder" title="Show in folder">${iconSvg('folder')}</button>
    `;
    el.querySelector('[data-act="folder"]').onclick = () =>
      invoke('cmd_open_path', { path: task.save_path, revealInFolder: true });
    listEl.appendChild(el);
  }
}

async function renderAdblockPanel(body) {
  const tab = activeTab();
  const host = tab && tab.url.startsWith('http') ? hostOf(tab.url) : '';
  const isAllow = host ? await invoke('cmd_is_site_allowlisted', { host }) : false;
  const isPopupAllow = host ? await invoke('cmd_is_popup_site_allowlisted', { host }) : false;
  const blockedPopup = tab && tab.last_blocked_popup ? tab.last_blocked_popup : '';

  body.innerHTML = `
    <div class="setting-group">
      <div class="setting-row">
        <div><strong>Ad &amp; Tracker Blocker</strong><div class="muted">Blokir script iklan, iframe, tracker &amp; banner</div></div>
        <input id="chk-adblock" type="checkbox" ${state.settings.adblock_enabled ? 'checked' : ''} />
      </div>
      <div class="setting-row">
        <div><strong>Smart Popup Blocker</strong><div class="muted">Blokir iklan tab baru / pop-under lintas domain</div></div>
        <input id="chk-popup" type="checkbox" ${state.settings.popup_blocker_enabled !== false ? 'checked' : ''} />
      </div>
    </div>
    <div class="setting-group">
      <div class="stat-value accent-purple">${state.adblock_blocked_count || 0}</div>
      <div class="muted">Iklan, tracker &amp; popup diblokir sesi ini</div>
    </div>
    ${blockedPopup ? `
    <div class="setting-group">
      <strong class="btn-with-icon">${iconSvg('ban')}<span>Popup Terakhir Diblokir</span></strong>
      <div class="panel-item-sub">${escapeHtml(blockedPopup)}</div>
      <div class="panel-row">
        <button id="btn-open-popup-once" class="btn">Buka Sekali</button>
        <button id="btn-allow-popup-open" class="btn btn-primary">Kecualikan &amp; Buka</button>
      </div>
    </div>` : ''}
    ${host ? `
    <div class="setting-group">
      <strong>Pengecualian untuk ${escapeHtml(host)}</strong>
      <div class="setting-row">
        <div class="muted">${isAllow ? 'Ad-Blocker dijeda di situs ini' : 'Ad-Blocker aktif di situs ini'}</div>
        <button id="btn-site-allow" class="btn">${isAllow ? 'Aktifkan' : 'Kecualikan'}</button>
      </div>
      <div class="setting-row">
        <div class="muted">${isPopupAllow ? 'Popup diizinkan di situs ini' : 'Popup diblokir di situs ini'}</div>
        <button id="btn-popup-site-allow" class="btn">${isPopupAllow ? 'Blokir Lagi' : 'Kecualikan'}</button>
      </div>
    </div>` : ''}
  `;

  $('chk-adblock').onchange = (e) => saveSettings({ adblock_enabled: e.target.checked });
  $('chk-popup').onchange = (e) => saveSettings({ popup_blocker_enabled: e.target.checked });
  if (blockedPopup && tab) {
    $('btn-open-popup-once').onclick = () => invoke('cmd_open_blocked_popup', { tabId: tab.id, alwaysAllowSite: false });
    $('btn-allow-popup-open').onclick = () => invoke('cmd_open_blocked_popup', { tabId: tab.id, alwaysAllowSite: true });
  }
  if (host) {
    $('btn-site-allow').onclick = async () => { await invoke('cmd_toggle_adblock_site', { host }); renderSidePanel(); };
    $('btn-popup-site-allow').onclick = async () => { await invoke('cmd_toggle_popup_site', { host }); renderSidePanel(); };
  }
}

function renderSettingsPanel(body) {
  body.innerHTML = `
    <div class="setting-group">
      <div class="setting-row">
        <label for="sel-theme"><strong>Appearance / Theme</strong></label>
        <select id="sel-theme" class="setting-select">
          <option value="dark">Dark Mode</option>
          <option value="light">Light Mode</option>
        </select>
      </div>
      <div class="setting-row">
        <label for="sel-search"><strong>Search Engine</strong></label>
        <select id="sel-search" class="setting-select">
          <option value="https://www.google.com/search?q=">Google</option>
          <option value="https://duckduckgo.com/?q=">DuckDuckGo</option>
          <option value="https://www.bing.com/search?q=">Bing</option>
          <option value="https://search.brave.com/search?q=">Brave Search</option>
        </select>
      </div>
      <div class="setting-row">
        <label for="sel-sleep"><strong>Auto Tab Sleeping</strong></label>
        <select id="sel-sleep" class="setting-select">
          <option value="1">1 minute</option>
          <option value="5">5 minutes</option>
          <option value="15">15 minutes</option>
          <option value="30">30 minutes</option>
          <option value="0">Disabled</option>
        </select>
      </div>
      <div class="setting-row">
        <label for="chk-bmbar"><strong>Show Bookmarks Bar</strong></label>
        <input id="chk-bmbar" type="checkbox" ${state.settings.show_bookmarks_bar ? 'checked' : ''} />
      </div>
    </div>
    <div class="setting-group">
      <strong>Actions</strong>
      <button id="btn-sleep-all" class="btn btn-primary btn-with-icon">${iconSvg('sleep')}<span>Sleep All Inactive Tabs Now</span></button>
      <button id="btn-open-about" class="btn btn-with-icon">${iconSvg('info')}<span>About ViBrowse</span></button>
    </div>
    <div class="setting-group">
      <strong>Chrome Keyboard Shortcuts</strong>
      <div class="shortcut-grid">
        <span>New Tab</span><span class="kbd">Ctrl+T</span>
        <span>New Incognito Tab</span><span class="kbd">Ctrl+Shift+N</span>
        <span>Close Tab</span><span class="kbd">Ctrl+W</span>
        <span>Reopen Closed Tab</span><span class="kbd">Ctrl+Shift+T</span>
        <span>Focus Address Bar</span><span class="kbd">Ctrl+L</span>
        <span>Reload</span><span class="kbd">Ctrl+R / F5</span>
        <span>Bookmark Page</span><span class="kbd">Ctrl+D</span>
        <span>History</span><span class="kbd">Ctrl+H</span>
        <span>Downloads</span><span class="kbd">Ctrl+J</span>
        <span>Toggle Bookmarks Bar</span><span class="kbd">Ctrl+Shift+B</span>
        <span>DevTools</span><span class="kbd">F12</span>
        <span>Fullscreen</span><span class="kbd">F11</span>
        <span>Zoom In / Out / Reset</span><span class="kbd">Ctrl + / - / 0</span>
      </div>
    </div>
  `;

  $('sel-theme').value = state.settings.theme || 'dark';
  $('sel-search').value = state.settings.search_engine;
  $('sel-sleep').value = String(state.settings.tab_sleep_minutes);

  const save = () => saveSettings({
    theme: $('sel-theme').value,
    search_engine: $('sel-search').value,
    tab_sleep_minutes: Number($('sel-sleep').value),
    show_bookmarks_bar: $('chk-bmbar').checked,
  });
  $('sel-theme').onchange = save;
  $('sel-search').onchange = save;
  $('sel-sleep').onchange = save;
  $('chk-bmbar').onchange = save;
  $('btn-sleep-all').onclick = () => invoke('cmd_sleep_inactive_tabs');
  $('btn-open-about').onclick = () => {
    invoke('cmd_set_panel', { panel: null });
    invoke('cmd_new_tab', { url: ABOUT_URL, incognito: false });
  };
}

const PANEL_RENDERERS = {
  bookmarks: { icon: 'bookmark', title: 'Bookmarks', render: renderBookmarksPanel },
  history: { icon: 'history', title: 'History', render: renderHistoryPanel },
  downloads: { icon: 'download', title: 'Downloads & Media', render: renderDownloadsPanel },
  adblock: { icon: 'shield', title: 'Ad & Popup Blocker', render: renderAdblockPanel },
  settings: { icon: 'settings', title: 'Settings', render: renderSettingsPanel },
};

async function renderSidePanel() {
  const panelEl = $('side-panel');
  const config = PANEL_RENDERERS[state.active_panel];
  if (!config) {
    panelEl.classList.add('hidden');
    return;
  }
  panelEl.classList.remove('hidden');
  $('panel-title').innerHTML = `${iconSvg(config.icon, 'ui-icon')}<span>${config.title}</span>`;
  await config.render($('panel-body'));
}

async function refreshBookmarks() {
  bookmarksCache = await invoke('cmd_list_bookmarks');
  renderBookmarksBar();
  renderNewTabPage();
}

function renderAll() {
  document.documentElement.dataset.theme = state.settings.theme === 'light' ? 'light' : 'dark';
  if (IS_PANEL_VIEW) {
    renderSidePanel();
    return;
  }
  renderTabs();
  renderToolbar();
  renderBookmarksBar();
  renderNewTabPage();
  renderDownloadsDashboard();
  renderSidePanel();
}

// ---- EVENT WIRING ----

$('btn-new-tab').onclick = () => invoke('cmd_new_tab', { url: null, incognito: false });
$('btn-new-incognito').onclick = () => invoke('cmd_new_tab', { url: null, incognito: true });
$('ram-pill').onclick = () => invoke('cmd_sleep_inactive_tabs');
$('btn-back').onclick = () => withTab((tab) => invoke('cmd_tab_back', { tabId: tab.id }));
$('btn-forward').onclick = () => withTab((tab) => invoke('cmd_tab_forward', { tabId: tab.id }));
$('btn-reload').onclick = () => withTab((tab) => invoke('cmd_tab_reload', { tabId: tab.id }));
$('btn-home').onclick = () => navigateActive(NEWTAB_URL);
$('btn-close-panel').onclick = () => invoke('cmd_set_panel', { panel: null });
$('btn-devtools').onclick = () => invoke('cmd_open_devtools');
$('btn-theme-toggle').onclick = () => saveSettings({ theme: state.settings.theme === 'light' ? 'dark' : 'light' });
$('about-open-dm').onclick = () => navigateActive(DOWNLOADS_URL);
$('about-open-settings').onclick = () => invoke('cmd_set_panel', { panel: 'settings' });

$('btn-bookmark').onclick = async () => {
  const tab = activeTab();
  if (!tab || isInternal(tab.url)) return;
  await invoke('cmd_toggle_bookmark', { title: tab.title || tab.url, url: tab.url });
  await refreshBookmarks();
};

document.querySelectorAll('[data-panel]').forEach((btn) => {
  btn.onclick = () => invoke('cmd_set_panel', { panel: btn.dataset.panel });
});

document.querySelectorAll('.dm-mode-btn').forEach((btn) => {
  btn.onclick = () => {
    dmAddMode = btn.dataset.mode;
    document.querySelectorAll('.dm-mode-btn').forEach((b) => b.classList.toggle('active', b === btn));
    $('dm-conns-select').classList.toggle('hidden', dmAddMode !== 'http');
    $('dm-quality-select').classList.toggle('hidden', dmAddMode !== 'video');
  };
});

document.querySelectorAll('.dm-filter-btn').forEach((btn) => {
  btn.onclick = () => {
    dmFilter = btn.dataset.filter;
    document.querySelectorAll('.dm-filter-btn').forEach((b) => b.classList.toggle('active', b === btn));
    renderDownloadsDashboard();
  };
});

$('dm-btn-clear-done').onclick = () => invoke('cmd_dm_clear_finished');

$('dm-add-form').onsubmit = async (e) => {
  e.preventDefault();
  const url = $('dm-url-input').value.trim();
  const errEl = $('dm-add-error');
  errEl.classList.add('hidden');
  if (!url) return;

  try {
    if (url.startsWith('magnet:') || url.toLowerCase().endsWith('.torrent') || dmAddMode === 'torrent') {
      await invoke('cmd_dm_start_torrent', { magnetOrUrl: url });
    } else if (dmAddMode === 'video') {
      await invoke('cmd_dm_start_video', { url, quality: $('dm-quality-select').value });
    } else {
      await invoke('cmd_dm_start_http', { url, connections: Number($('dm-conns-select').value), filename: null });
    }
    $('dm-url-input').value = '';
  } catch (err) {
    errEl.textContent = String(err);
    errEl.classList.remove('hidden');
  }
};

// Omnibox navigation & autocomplete
const omnibox = $('omnibox');
const dropdown = $('omnibox-dropdown');

omnibox.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') {
    dropdown.classList.add('hidden');
    navigateActive(omnibox.value);
    omnibox.blur();
  } else if (e.key === 'Escape') {
    dropdown.classList.add('hidden');
    omnibox.blur();
  }
});
omnibox.addEventListener('focus', () => omnibox.select());
omnibox.addEventListener('blur', () => setTimeout(() => dropdown.classList.add('hidden'), 160));

omnibox.addEventListener('input', async () => {
  const query = omnibox.value.trim();
  const tab = activeTab();
  if (!query || (tab && tab.url !== NEWTAB_URL)) {
    dropdown.classList.add('hidden');
    return;
  }
  const items = await invoke('cmd_search_omnibox', { query });
  if (!items.length) {
    dropdown.classList.add('hidden');
    return;
  }
  dropdown.innerHTML = '';
  for (const item of items) {
    const el = document.createElement('div');
    el.className = 'suggest-item';
    el.innerHTML = `
      ${iconSvg(item.kind === 'bookmark' ? 'star' : 'history')}
      <div class="suggest-text">
        <div class="suggest-title">${escapeHtml(item.title)}</div>
        <div class="suggest-url">${escapeHtml(item.url)}</div>
      </div>
    `;
    el.onmousedown = () => { navigateActive(item.url); dropdown.classList.add('hidden'); };
    dropdown.appendChild(el);
  }
  dropdown.classList.remove('hidden');
});

$('ntp-search-form').onsubmit = (e) => {
  e.preventDefault();
  const query = $('ntp-search-input').value.trim();
  if (query) {
    navigateActive(query);
    $('ntp-search-input').value = '';
  }
};

// ---- CHROME KEYBOARD SHORTCUTS ----
const SHORTCUTS = [
  [(e, c, k) => c && !e.shiftKey && k === 't', () => invoke('cmd_new_tab', { url: null, incognito: false })],
  [(e, c, k) => c && e.shiftKey && k === 'n', () => invoke('cmd_new_tab', { url: null, incognito: true })],
  [(e, c, k) => c && !e.shiftKey && k === 'w', () => withTab((tab) => invoke('cmd_close_tab', { tabId: tab.id }))],
  [(e, c, k) => c && e.shiftKey && k === 't', () => invoke('cmd_reopen_closed_tab')],
  [(e, c, k) => (c && k === 'l') || (e.altKey && k === 'd'), () => { omnibox.focus(); omnibox.select(); }],
  [(e, c, k) => (c && k === 'r') || e.key === 'F5', () => withTab((tab) => invoke('cmd_tab_reload', { tabId: tab.id }))],
  [(e) => e.altKey && e.key === 'ArrowLeft', () => withTab((tab) => invoke('cmd_tab_back', { tabId: tab.id }))],
  [(e) => e.altKey && e.key === 'ArrowRight', () => withTab((tab) => invoke('cmd_tab_forward', { tabId: tab.id }))],
  [(e, c, k) => c && !e.shiftKey && k === 'd', () => $('btn-bookmark').click()],
  [(e, c, k) => c && !e.shiftKey && k === 'h', () => invoke('cmd_set_panel', { panel: 'history' })],
  [(e, c, k) => c && !e.shiftKey && k === 'j', () => invoke('cmd_set_panel', { panel: 'downloads' })],
  [(e, c, k) => c && e.shiftKey && k === 'b', () => saveSettings({ show_bookmarks_bar: !state.settings.show_bookmarks_bar })],
  [(e) => e.key === 'F11', () => invoke('cmd_toggle_fullscreen')],
  [(e, c, k) => e.key === 'F12' || (c && e.shiftKey && k === 'i'), () => invoke('cmd_open_devtools')],
  [(e, c) => c && (e.key === '=' || e.key === '+'), () => withTab((tab) => invoke('cmd_tab_zoom', { tabId: tab.id, delta: 0.1, reset: false }))],
  [(e, c) => c && e.key === '-', () => withTab((tab) => invoke('cmd_tab_zoom', { tabId: tab.id, delta: -0.1, reset: false }))],
  [(e, c) => c && e.key === '0', () => withTab((tab) => invoke('cmd_tab_zoom', { tabId: tab.id, delta: 0.0, reset: true }))],
  [(e, c) => c && e.key === 'Tab', (e) => withTab((tab) => {
    if (state.tabs.length < 2) return;
    const idx = state.tabs.findIndex((x) => x.id === tab.id);
    const nextIdx = (idx + (e.shiftKey ? -1 : 1) + state.tabs.length) % state.tabs.length;
    invoke('cmd_switch_tab', { tabId: state.tabs[nextIdx].id });
  })],
  [(e, c) => c && /^[1-9]$/.test(e.key), (e) => {
    const n = Number(e.key);
    const target = n === 9 ? state.tabs[state.tabs.length - 1] : state.tabs[n - 1];
    if (target) invoke('cmd_switch_tab', { tabId: target.id });
  }],
];

window.addEventListener('keydown', (e) => {
  const ctrl = e.ctrlKey || e.metaKey;
  const key = e.key.toLowerCase();
  const match = SHORTCUTS.find(([pred]) => pred(e, ctrl, key));
  if (match) {
    e.preventDefault();
    match[1](e);
  }
});

// ---- INIT ----
async function init() {
  await listen('browser-state', (ev) => { state = ev.payload; renderAll(); });
  await listen('dm-updated', (ev) => {
    dmTasks = ev.payload;
    if (!IS_PANEL_VIEW) renderDownloadsDashboard();
    if (state.active_panel === 'downloads') renderSidePanel();
  });

  state = await invoke('get_state');
  dmTasks = await invoke('cmd_dm_list');
  dmEngineStatus = await invoke('cmd_dm_engine_status');
  await refreshBookmarks();
  renderAll();
}

init();
