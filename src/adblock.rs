use crate::util::extract_host;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

/// Built-in lightweight ad, tracker & streaming pop-under domain list (O(1) lookup).
const DEFAULT_BLOCKED_DOMAINS: &[&str] = &[
    // Major ad & analytics networks
    "doubleclick.net",
    "googlesyndication.com",
    "googleadservices.com",
    "google-analytics.com",
    "googletagmanager.com",
    "googletagservices.com",
    "adservice.google.com",
    "pagead2.googlesyndication.com",
    "tpc.googlesyndication.com",
    "ads.youtube.com",
    "adnxs.com",
    "adsrvr.org",
    "amazon-adsystem.com",
    "criteo.com",
    "criteo.net",
    "outbrain.com",
    "taboola.com",
    "pubmatic.com",
    "rubiconproject.com",
    "openx.net",
    "casalemedia.com",
    "indexww.com",
    "scorecardresearch.com",
    "quantserve.com",
    "hotjar.com",
    "mixpanel.com",
    "segment.io",
    "segment.com",
    "amplitude.com",
    "fullstory.com",
    "crazyegg.com",
    "mouseflow.com",
    "clarity.ms",
    "connect.facebook.net",
    "pixel.facebook.com",
    "analytics.tiktok.com",
    "ads.tiktok.com",
    "ads.twitter.com",
    "analytics.twitter.com",
    "ads.linkedin.com",
    "px.ads.linkedin.com",
    "bat.bing.com",
    "adroll.com",
    "bidswitch.net",
    "demdex.net",
    "krxd.net",
    "omtrdc.net",
    "moatads.com",
    "doubleverify.com",
    "adsafeprotected.com",
    "serving-sys.com",
    "sizmek.com",
    "smartadserver.com",
    "yieldmo.com",
    "sharethrough.com",
    "33across.com",
    "triplelift.com",
    "media.net",
    "revcontent.com",
    "mgid.com",
    "zedo.com",
    "adcolony.com",
    "applovin.com",
    "unityads.unity3d.com",
    "chartboost.com",
    "flurry.com",
    "branch.io",
    "appsflyer.com",
    "adjust.com",
    "sentry.io",
    "newrelic.com",
    "nr-data.net",
    // Streaming / Donghua / Anime pop-under & aggressive ad networks
    "propellerads.com",
    "monetag.com",
    "alwingulla.com",
    "dolohen.com",
    "glizauvo.net",
    "thaudray.com",
    "zuphaims.com",
    "propu.sh",
    "pushub.net",
    "pushame.com",
    "notifpush.com",
    "popads.net",
    "popcash.net",
    "popmyads.com",
    "adsterra.com",
    "highperformanceformat.com",
    "highcpmgate.com",
    "highcpmrevenuegate.com",
    "effectivegatecpm.com",
    "profitablecpmrate.com",
    "topcreativeformat.com",
    "revenuenetworkcpm.com",
    "hilltopads.net",
    "hilltopads.com",
    "clickadu.com",
    "adcash.com",
    "acscdn.com",
    "exoclick.com",
    "exosrv.com",
    "realsrv.com",
    "tsyndicate.com",
    "juicyads.com",
    "trafficjunky.net",
    "trafficstars.com",
    "galaksion.com",
    "richads.com",
    "admaven.com",
    "ad-maven.com",
    "bidvertiser.com",
    "revenuehits.com",
    "histats.com",
    "a-ads.com",
    "coinzilla.com",
    "cointraffic.io",
    "onclkds.com",
    "onclickalgo.com",
    "onclickmax.com",
    "onclickperformance.com",
    "syndication.realsrv.com",
    "acint.net",
    "betterbannerscloud.com",
    "dtscout.com",
    "adskeeper.com",
    "adskeeper.co.uk",
    "yllix.com",
    "adf.ly",
    "bc.vc",
    "ouo.io",
    "ouo.press",
    "shrinkme.io",
    "exe.io",
    "fc.lc",
    "linkvertise.com",
    "shorte.st",
    "s10.histats.com",
    "s4.histats.com",
    "mc.yandex.ru",
];

/// URL path/query fragments strongly indicative of ad scripts, pop-unders, or trackers.
const BLOCKED_URL_PATTERNS: &[&str] = &[
    "/popunder",
    "/pop.js",
    "/popads",
    "/clickunder",
    "adsterra",
    "propellerads",
    "monetag",
    "hilltopads",
    "/pagead/",
    "/ads/banner",
    "/adserver/",
    "doubleclick.net",
    "googlesyndication.com",
    "vast.xml",
    "vpaid.js",
    "invoke.js",
    "/atoptions",
    "histats.com/js",
];

/// Trusted auth/login hosts that should never be blocked by cross-origin popup protection.
const SAFE_POPUP_HOSTS: &[&str] = &[
    "accounts.google.com",
    "github.com",
    "login.microsoftonline.com",
    "appleid.apple.com",
    "discord.com",
    "oauth.telegram.org",
];

/// Extracts a simple registrable root domain (last two labels, e.g. `anichin.cafe`).
fn root_domain(host: &str) -> String {
    let parts: Vec<&str> = host.split('.').filter(|p| !p.is_empty()).collect();
    if parts.len() >= 2 {
        format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1])
    } else {
        host.to_string()
    }
}

pub struct AdBlocker {
    blocked_domains: HashSet<&'static str>,
    blocked_domains_json: String,
    allowlist_hosts: RwLock<HashSet<String>>,
    popup_allowlist_hosts: RwLock<HashSet<String>>,
    blocked_count: AtomicU64,
}

impl AdBlocker {
    pub fn new() -> Self {
        let blocked_domains: HashSet<&'static str> =
            DEFAULT_BLOCKED_DOMAINS.iter().copied().collect();
        let blocked_domains_json =
            serde_json::to_string(DEFAULT_BLOCKED_DOMAINS).unwrap_or_else(|_| "[]".to_string());
        Self {
            blocked_domains,
            blocked_domains_json,
            allowlist_hosts: RwLock::new(HashSet::new()),
            popup_allowlist_hosts: RwLock::new(HashSet::new()),
            blocked_count: AtomicU64::new(0),
        }
    }

    fn matches_blocked_host(&self, host: &str) -> bool {
        let mut current = host;
        loop {
            if self.blocked_domains.contains(current) {
                return true;
            }
            match current.find('.') {
                Some(idx) => current = &current[idx + 1..],
                None => return false,
            }
        }
    }

    fn matches_url_pattern(raw_url: &str) -> bool {
        let lower = raw_url.to_lowercase();
        BLOCKED_URL_PATTERNS.iter().any(|p| lower.contains(p))
    }

    /// Checks whether a top-level navigation URL should be blocked.
    pub fn should_block_url(&self, raw_url: &str, adblock_enabled: bool) -> bool {
        if !adblock_enabled {
            return false;
        }
        let Some(host) = extract_host(raw_url) else {
            return false;
        };
        if self.is_allowlisted(&host) {
            return false;
        }
        if self.matches_blocked_host(&host) || Self::matches_url_pattern(raw_url) {
            self.blocked_count.fetch_add(1, Ordering::Relaxed);
            return true;
        }
        false
    }

    /// Checks whether a subresource request (script, iframe, xhr, image) on `page_host` should be blocked.
    pub fn should_block_subresource(
        &self,
        raw_url: &str,
        page_host: &str,
        adblock_enabled: bool,
    ) -> bool {
        if !adblock_enabled || (!page_host.is_empty() && self.is_allowlisted(page_host)) {
            return false;
        }
        let Some(req_host) = extract_host(raw_url) else {
            return false;
        };
        if self.matches_blocked_host(&req_host) || Self::matches_url_pattern(raw_url) {
            self.blocked_count.fetch_add(1, Ordering::Relaxed);
            return true;
        }
        false
    }

    /// Decides whether a new-tab/popup request from `source_url` to `target_url` should be blocked.
    pub fn should_block_popup(
        &self,
        source_url: &str,
        target_url: &str,
        adblock_enabled: bool,
        popup_blocker_enabled: bool,
    ) -> bool {
        let source_host = extract_host(source_url).unwrap_or_default();

        if adblock_enabled
            && (source_host.is_empty() || !self.is_allowlisted(&source_host))
            && self.should_block_url(target_url, true)
        {
            return true;
        }

        if !popup_blocker_enabled
            || (!source_host.is_empty() && self.is_popup_allowlisted(&source_host))
        {
            return false;
        }

        let Some(target_host) = extract_host(target_url) else {
            return false;
        };

        if SAFE_POPUP_HOSTS
            .iter()
            .any(|safe| target_host == *safe || target_host.ends_with(&format!(".{}", safe)))
        {
            return false;
        }

        if !source_host.is_empty() && root_domain(&source_host) != root_domain(&target_host) {
            self.blocked_count.fetch_add(1, Ordering::Relaxed);
            return true;
        }

        false
    }

    pub fn total_blocked(&self) -> u64 {
        self.blocked_count.load(Ordering::Relaxed)
    }

    pub fn toggle_allowlist(&self, host: &str) -> bool {
        toggle_set_entry(&self.allowlist_hosts, host)
    }

    pub fn is_allowlisted(&self, host: &str) -> bool {
        self.allowlist_hosts
            .read()
            .unwrap()
            .contains(&host.trim().to_lowercase())
    }

    pub fn toggle_popup_allowlist(&self, host: &str) -> bool {
        toggle_set_entry(&self.popup_allowlist_hosts, host)
    }

    pub fn allow_popup_site(&self, host: &str) {
        let key = host.trim().to_lowercase();
        if !key.is_empty() {
            self.popup_allowlist_hosts.write().unwrap().insert(key);
        }
    }

    pub fn is_popup_allowlisted(&self, host: &str) -> bool {
        self.popup_allowlist_hosts
            .read()
            .unwrap()
            .contains(&host.trim().to_lowercase())
    }

    pub fn blocked_domains_json(&self) -> &str {
        &self.blocked_domains_json
    }
}

fn toggle_set_entry(lock: &RwLock<HashSet<String>>, host: &str) -> bool {
    let key = host.trim().to_lowercase();
    let mut set = lock.write().unwrap();
    if set.remove(&key) {
        false
    } else {
        set.insert(key);
        true
    }
}

/// Generates the lightweight initialization script injected into every content tab webview
/// (AdBlock + Smart Popup Blocker + IDM-style Floating Video Downloader button).
pub fn build_tab_init_script(
    adblock_enabled: bool,
    popup_blocker_enabled: bool,
    blocked_domains_json: &str,
) -> String {
    format!(
        r#"
(function() {{
  if (window.__VIBROWSE_ADBLOCK__) return;
  window.__VIBROWSE_ADBLOCK__ = true;

  const ADBLOCK_ON = {adblock_enabled};
  const POPUP_BLOCK_ON = {popup_blocker_enabled};
  const BLOCKED_DOMAINS = new Set({blocked_domains_json});
  const ORIG_WINDOW_OPEN = window.open ? window.open.bind(window) : null;

  function isBlockedHost(urlStr) {{
    try {{
      const u = new URL(urlStr, location.href);
      let h = u.hostname.toLowerCase();
      while (h) {{
        if (BLOCKED_DOMAINS.has(h)) return true;
        const idx = h.indexOf('.');
        if (idx === -1) break;
        h = h.slice(idx + 1);
      }}
    }} catch (_) {{}}
    return false;
  }}

  if (ADBLOCK_ON) {{
    const origFetch = window.fetch;
    if (origFetch) {{
      window.fetch = function(input) {{
        const url = typeof input === 'string' ? input : (input && input.url ? input.url : '');
        if (url && isBlockedHost(url)) {{
          return Promise.resolve(new Response('', {{ status: 204, statusText: 'Blocked by ViBrowse' }}));
        }}
        return origFetch.apply(this, arguments);
      }};
    }}

    const origOpen = XMLHttpRequest.prototype.open;
    XMLHttpRequest.prototype.open = function(method, url) {{
      if (url && isBlockedHost(String(url))) {{
        return origOpen.call(this, method, 'data:text/plain,');
      }}
      return origOpen.apply(this, arguments);
    }};

    const style = document.createElement('style');
    style.id = 'vibrowse-adblock-css';
    style.textContent = `
      ins.adsbygoogle,
      iframe[src*="doubleclick.net"],
      iframe[src*="googlesyndication.com"],
      iframe[src*="adsterra"],
      iframe[src*="exoclick"],
      iframe[src*="juicyads"],
      iframe[src*="popads"],
      iframe[id^="google_ads_iframe"],
      div[id^="div-gpt-ad"],
      .ad-container, .ad-banner, .ad-slot, .sponsored-ad,
      .floating-ads, .sticky-ads, .banner-ads, .iklan, .ads-box,
      #taboola-below-article-thumbnails, .OUTBRAIN {{
        display: none !important;
        visibility: hidden !important;
        height: 0 !important;
        min-height: 0 !important;
        pointer-events: none !important;
      }}
    `;
    (document.head || document.documentElement).appendChild(style);
  }}

  if (POPUP_BLOCK_ON) {{
    const origWindowOpen = window.open;
    window.open = function(url) {{
      if (url && isBlockedHost(String(url))) return null;
      return origWindowOpen.apply(this, arguments);
    }};

    document.addEventListener('mousedown', function(e) {{
      const el = e.target;
      if (!el || !el.getBoundingClientRect) return;
      const st = window.getComputedStyle(el);
      const z = parseInt(st.zIndex, 10) || 0;
      const rect = el.getBoundingClientRect();
      const coversScreen = rect.width >= window.innerWidth * 0.75 && rect.height >= window.innerHeight * 0.75;
      const isTransparent = parseFloat(st.opacity) < 0.1 || st.backgroundColor === 'rgba(0, 0, 0, 0)';
      const isTrap = (st.position === 'fixed' || st.position === 'absolute')
        && z >= 9999
        && coversScreen
        && isTransparent
        && !el.querySelector('video, iframe, input, form');
      if (isTrap) {{
        e.preventDefault();
        e.stopPropagation();
        el.remove();
      }}
    }}, true);
  }}

  // ---- IDM-STYLE FLOATING VIDEO DOWNLOAD BUTTON ----
  (function initFloatingDownloader() {{
    const PLATFORM_RE = /(youtube\.com\/(watch|shorts)|youtu\.be\/|tiktok\.com\/.*\/video|instagram\.com\/(reel|p|tv)\/|facebook\.com\/.*(videos|reel|watch)|fb\.watch\/|vimeo\.com\/\d+|dailymotion\.com\/video|twitch\.tv\/videos|bilibili\.com\/video|x\.com\/.*\/status|twitter\.com\/.*\/status)/i;
    let host = null;
    let wrap = null;
    let menu = null;
    let mainBtn = null;
    let labelEl = null;
    let dismissedHref = null;
    let lastSyncedHref = null;
    let activeVideo = null;
    let menuOpen = false;

    function isVideoPlatformUrl(href) {{
      return PLATFORM_RE.test(href || '');
    }}

    function resolveTargetUrl(video) {{
      const href = location.href;
      const host = location.hostname.toLowerCase();

      // 1. Twitter / X: on timeline/home feeds, extract the tweet's exact /status/<id> link
      if (host.includes('x.com') || host.includes('twitter.com')) {{
        if (/\/status\/\d+/i.test(href)) {{
          return href.split('?')[0];
        }}
        if (video) {{
          const art = video.closest('article');
          if (art) {{
            const timeLink = art.querySelector('a[href*="/status/"] time');
            const a = timeLink ? timeLink.closest('a') : art.querySelector('a[href*="/status/"]');
            if (a && a.href) return a.href.split('?')[0];
          }}
        }}
      }}

      // 2. YouTube: normalize /watch?v=ID (strip radio/mix list params) or extract preview link
      if (host.includes('youtube.com') || host === 'youtu.be') {{
        try {{
          const u = new URL(href);
          const vid = u.searchParams.get('v');
          if (u.pathname === '/watch' && vid) {{
            return 'https://www.youtube.com/watch?v=' + encodeURIComponent(vid);
          }}
          if (u.pathname.startsWith('/shorts/')) {{
            return u.origin + u.pathname;
          }}
        }} catch (_) {{}}
        if (video) {{
          const card = video.closest('ytd-rich-item-renderer, ytd-video-renderer, ytd-compact-video-renderer, ytd-reel-video-renderer');
          const a = card ? card.querySelector('a[href*="/watch?v="], a[href*="/shorts/"]') : null;
          if (a && a.href) return a.href;
        }}
      }}

      if (isVideoPlatformUrl(href)) return href;
      const src = video ? (video.currentSrc || video.src || '') : '';
      if (src && /^https?:\/\//i.test(src)) return src;
      return href;
    }}

    function triggerDownload(quality) {{
      const target = resolveTargetUrl(activeVideo);
      if (!target) return;
      const dlUrl = 'vibrowse-dl://video?url=' + encodeURIComponent(target) + '&quality=' + encodeURIComponent(quality || 'best');
      if (ORIG_WINDOW_OPEN) {{
        ORIG_WINDOW_OPEN(dlUrl, '_blank');
      }} else {{
        window.open(dlUrl, '_blank');
      }}
      menuOpen = false;
      if (menu) menu.style.display = 'none';
      if (labelEl) {{
        labelEl.textContent = 'Mengunduh...';
        setTimeout(() => {{
          if (labelEl) labelEl.textContent = 'Download Video';
        }}, 2200);
      }}
    }}

    function ensureUi() {{
      if (host && host.isConnected) return;
      host = document.createElement('div');
      host.id = 'vibrowse-idm-host';
      host.style.cssText = 'position:fixed;top:0;left:0;z-index:2147483646;pointer-events:none;';
      const shadow = host.attachShadow({{ mode: 'closed' }});
      shadow.innerHTML = `
        <style>
          .vb-wrap {{
            position: fixed;
            display: none;
            align-items: center;
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            font-size: 12px;
            pointer-events: auto;
            user-select: none;
          }}
          .vb-pill {{
            display: inline-flex;
            align-items: stretch;
            background: rgba(15, 23, 42, 0.92);
            border: 1px solid rgba(59, 130, 246, 0.55);
            border-radius: 9px;
            box-shadow: 0 6px 20px rgba(0, 0, 0, 0.45);
            backdrop-filter: blur(8px);
            overflow: hidden;
          }}
          .vb-btn {{
            border: none;
            background: transparent;
            color: #f8fafc;
            cursor: pointer;
            display: inline-flex;
            align-items: center;
            gap: 6px;
            padding: 6px 10px;
            font-size: 12px;
            font-weight: 600;
            line-height: 1;
            transition: background 0.15s;
          }}
          .vb-btn:hover {{ background: rgba(59, 130, 246, 0.3); }}
          .vb-btn.primary {{
            background: linear-gradient(135deg, rgba(37, 99, 235, 0.85), rgba(79, 70, 229, 0.85));
          }}
          .vb-btn.primary:hover {{
            background: linear-gradient(135deg, #2563eb, #4f46e5);
          }}
          .vb-sep {{
            width: 1px;
            background: rgba(255, 255, 255, 0.16);
          }}
          .vb-close {{
            padding: 6px 7px;
            color: #94a3b8;
          }}
          .vb-close:hover {{ color: #fff; background: rgba(239, 68, 68, 0.35); }}
          .vb-menu {{
            position: absolute;
            top: calc(100% + 5px);
            right: 0;
            min-width: 220px;
            max-width: 290px;
            background: rgba(15, 23, 42, 0.97);
            border: 1px solid rgba(148, 163, 184, 0.28);
            border-radius: 10px;
            box-shadow: 0 10px 28px rgba(0, 0, 0, 0.6);
            padding: 6px;
            display: none;
            flex-direction: column;
            gap: 2px;
          }}
          .vb-menu-title {{
            font-size: 11px;
            font-weight: 600;
            color: #93c5fd;
            padding: 4px 8px 6px;
            border-bottom: 1px solid rgba(255, 255, 255, 0.1);
            margin-bottom: 3px;
            overflow: hidden;
            text-overflow: ellipsis;
            white-space: nowrap;
          }}
          .vb-opt {{
            border: none;
            background: transparent;
            color: #e2e8f0;
            text-align: left;
            padding: 7px 10px;
            border-radius: 6px;
            font-size: 12px;
            font-weight: 500;
            cursor: pointer;
            display: flex;
            justify-content: space-between;
            align-items: center;
          }}
          .vb-opt:hover {{ background: rgba(59, 130, 246, 0.28); color: #fff; }}
          .vb-tag {{ font-size: 10px; color: #93c5fd; font-weight: 600; }}
          svg {{ width: 13px; height: 13px; stroke: currentColor; fill: none; stroke-width: 2.2; stroke-linecap: round; stroke-linejoin: round; }}
        </style>
        <div class="vb-wrap" id="vb-wrap">
          <div class="vb-pill">
            <button class="vb-btn primary" id="vb-dl" title="Unduh video ini dengan ViBrowse Download Manager">
              <svg viewBox="0 0 24 24"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
              <span id="vb-lbl">Download Video</span>
            </button>
            <div class="vb-sep"></div>
            <button class="vb-btn" id="vb-caret" title="Pilih kualitas video">
              <svg viewBox="0 0 24 24"><polyline points="6 9 12 15 18 9"/></svg>
            </button>
            <div class="vb-sep"></div>
            <button class="vb-btn vb-close" id="vb-close" title="Sembunyikan tombol ini">
              <svg viewBox="0 0 24 24"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
            </button>
          </div>
          <div class="vb-menu" id="vb-menu">
            <div class="vb-menu-title" id="vb-title">Video Terdeteksi</div>
            <button class="vb-opt" data-q="best"><span>Best Quality</span><span class="vb-tag">MP4</span></button>
            <button class="vb-opt" data-q="1080p"><span>Full HD 1080p</span><span class="vb-tag">MP4</span></button>
            <button class="vb-opt" data-q="720p"><span>HD 720p</span><span class="vb-tag">MP4</span></button>
            <button class="vb-opt" data-q="480p"><span>SD 480p</span><span class="vb-tag">MP4</span></button>
            <button class="vb-opt" data-q="audio"><span>Audio Only</span><span class="vb-tag">M4A</span></button>
          </div>
        </div>
      `;

      wrap = shadow.getElementById('vb-wrap');
      menu = shadow.getElementById('vb-menu');
      mainBtn = shadow.getElementById('vb-dl');
      labelEl = shadow.getElementById('vb-lbl');
      const titleEl = shadow.getElementById('vb-title');

      mainBtn.addEventListener('click', (e) => {{
        e.stopPropagation();
        triggerDownload('best');
      }});
      shadow.getElementById('vb-caret').addEventListener('click', (e) => {{
        e.stopPropagation();
        if (titleEl) {{
          titleEl.textContent = (document.title || 'Video Terdeteksi').replace(/\s*-\s*YouTube$/i, '');
        }}
        menuOpen = !menuOpen;
        menu.style.display = menuOpen ? 'flex' : 'none';
      }});
      shadow.getElementById('vb-close').addEventListener('click', (e) => {{
        e.stopPropagation();
        dismissedHref = resolveTargetUrl(activeVideo) || location.href;
        wrap.style.display = 'none';
        menuOpen = false;
        menu.style.display = 'none';
      }});
      shadow.querySelectorAll('.vb-opt').forEach((btn) => {{
        btn.addEventListener('click', (e) => {{
          e.stopPropagation();
          triggerDownload(btn.getAttribute('data-q') || 'best');
        }});
      }});
      document.addEventListener('click', () => {{
        if (menuOpen) {{
          menuOpen = false;
          if (menu) menu.style.display = 'none';
        }}
      }});
      (document.documentElement || document.body).appendChild(host);
    }}

    function findPrimaryVideo() {{
      const videos = document.querySelectorAll('video');
      let best = null;
      let bestArea = 0;
      for (const v of videos) {{
        const r = v.getBoundingClientRect();
        if (r.width < 220 || r.height < 120) continue;
        if (r.bottom < 40 || r.top > window.innerHeight - 40) continue;
        const hasMedia = isVideoPlatformUrl(location.href) || Boolean(v.currentSrc || v.src || v.readyState > 0);
        if (!hasMedia) continue;
        const area = r.width * r.height;
        if (area > bestArea) {{
          bestArea = area;
          best = v;
        }}
      }}
      return best;
    }}

    function updatePosition() {{
      if (isVideoPlatformUrl(location.href) && location.href !== lastSyncedHref) {{
        lastSyncedHref = location.href;
        const cleanTitle = (document.title || 'Video').replace(/\s*-\s*YouTube$/i, '');
        const syncUrl = 'vibrowse-dl://sync?url=' + encodeURIComponent(location.href) + '&title=' + encodeURIComponent(cleanTitle);
        if (ORIG_WINDOW_OPEN) ORIG_WINDOW_OPEN(syncUrl, '_blank');
      }}
      if (document.fullscreenElement) {{
        if (wrap) wrap.style.display = 'none';
        return;
      }}
      const video = findPrimaryVideo();
      activeVideo = video;
      if (!video) {{
        if (wrap) wrap.style.display = 'none';
        return;
      }}
      const key = resolveTargetUrl(video) || location.href;
      if (dismissedHref === key) {{
        if (wrap) wrap.style.display = 'none';
        return;
      }}
      ensureUi();
      const r = video.getBoundingClientRect();
      // Anchor at top-right corner of the video player (with close button to hide anytime)
      const top = Math.max(8, Math.min(window.innerHeight - 54, r.top + 8));
      const left = Math.max(8, Math.min(window.innerWidth - 208, r.right - 204));
      wrap.style.top = top + 'px';
      wrap.style.left = left + 'px';
      wrap.style.display = 'flex';
    }}

    setInterval(updatePosition, 800);
    window.addEventListener('scroll', updatePosition, {{ passive: true }});
    window.addEventListener('resize', updatePosition, {{ passive: true }});
    document.addEventListener('fullscreenchange', updatePosition);
  }})();
}})();
"#
    )
}
