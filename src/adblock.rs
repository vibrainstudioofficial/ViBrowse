use crate::util::extract_host;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use url::Url;

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
    blocked_domains: RwLock<HashSet<String>>,
    allowlist_hosts: RwLock<HashSet<String>>,
    popup_allowlist_hosts: RwLock<HashSet<String>>,
    blocked_count: AtomicU64,
}

impl AdBlocker {
    pub fn new() -> Self {
        let mut set = HashSet::with_capacity(DEFAULT_BLOCKED_DOMAINS.len());
        for d in DEFAULT_BLOCKED_DOMAINS {
            set.insert((*d).to_string());
        }
        Self {
            blocked_domains: RwLock::new(set),
            allowlist_hosts: RwLock::new(HashSet::new()),
            popup_allowlist_hosts: RwLock::new(HashSet::new()),
            blocked_count: AtomicU64::new(0),
        }
    }

    fn matches_blocked_host(&self, host: &str) -> bool {
        let blocked = self.blocked_domains.read().unwrap();
        let mut current: &str = host;
        loop {
            if blocked.contains(current) {
                return true;
            }
            match current.find('.') {
                Some(idx) => current = &current[idx + 1..],
                None => break,
            }
        }
        false
    }

    fn matches_url_pattern(raw_url: &str) -> bool {
        let lower = raw_url.to_lowercase();
        BLOCKED_URL_PATTERNS.iter().any(|p| lower.contains(p))
    }

    /// Checks whether a top-level navigation URL should be blocked.
    pub fn should_block_url(&self, raw_url: &str, enabled: bool) -> bool {
        if !enabled {
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
    pub fn should_block_subresource(&self, raw_url: &str, page_host: &str, enabled: bool) -> bool {
        if !enabled {
            return false;
        }
        if !page_host.is_empty() && self.is_allowlisted(page_host) {
            return false;
        }
        let parsed = match Url::parse(raw_url) {
            Ok(u) => u,
            Err(_) => return false,
        };
        let scheme = parsed.scheme();
        if scheme != "http" && scheme != "https" {
            return false;
        }
        let req_host = match parsed.host_str() {
            Some(h) => h.to_lowercase(),
            None => return false,
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

        // Always block known ad/tracker URLs when adblock is enabled (unless site is allowlisted)
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

        // Allow trusted OAuth/login popups
        if SAFE_POPUP_HOSTS
            .iter()
            .any(|safe| target_host == *safe || target_host.ends_with(&format!(".{}", safe)))
        {
            return false;
        }

        // Cross-origin popup from a webpage is the #1 vector for streaming/donghua pop-under ads.
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
        let host_lower = host.trim().to_lowercase();
        if !host_lower.is_empty() {
            self.popup_allowlist_hosts
                .write()
                .unwrap()
                .insert(host_lower);
        }
    }

    pub fn is_popup_allowlisted(&self, host: &str) -> bool {
        self.popup_allowlist_hosts
            .read()
            .unwrap()
            .contains(&host.trim().to_lowercase())
    }

    pub fn blocked_domains_json(&self) -> String {
        let domains = self.blocked_domains.read().unwrap();
        let list: Vec<&String> = domains.iter().collect();
        serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string())
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

/// Generates the lightweight initialization script injected into every content tab webview:
/// 1. Network-level fetch/XHR ad & tracker blocking
/// 2. Anti pop-under `window.open` & invisible full-screen click-overlay neutralizer
/// 3. Cosmetic ad-hiding CSS (zero layout thrash)
pub fn build_tab_init_script(
    adblock_enabled: bool,
    popup_blocker_enabled: bool,
    blocked_domains_json: &str,
) -> String {
    if !adblock_enabled && !popup_blocker_enabled {
        return String::new();
    }

    format!(
        r#"
(function() {{
  if (window.__VIBROWSE_ADBLOCK__) return;
  window.__VIBROWSE_ADBLOCK__ = true;

  const ADBLOCK_ON = {adblock_on};
  const POPUP_BLOCK_ON = {popup_on};
  const BLOCKED_DOMAINS = new Set({blocked_domains_json});

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
    // 1. Intercept fetch()
    const origFetch = window.fetch;
    if (origFetch) {{
      window.fetch = function(input, init) {{
        const url = typeof input === 'string' ? input : (input && input.url ? input.url : '');
        if (url && isBlockedHost(url)) {{
          return Promise.resolve(new Response('', {{ status: 204, statusText: 'Blocked by ViBrowse' }}));
        }}
        return origFetch.apply(this, arguments);
      }};
    }}

    // 2. Intercept XMLHttpRequest
    const origOpen = XMLHttpRequest.prototype.open;
    XMLHttpRequest.prototype.open = function(method, url) {{
      if (url && isBlockedHost(String(url))) {{
        return origOpen.call(this, method, 'data:text/plain,');
      }}
      return origOpen.apply(this, arguments);
    }};

    // 3. Lightweight cosmetic ad hiding
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
    // 4. Neutralize invisible click-hijack pop-under overlays on streaming sites
    const origWindowOpen = window.open;
    window.open = function(url) {{
      if (url && isBlockedHost(String(url))) {{
        return null;
      }}
      return origWindowOpen.apply(this, arguments);
    }};

    // Remove transparent full-screen click-trap <a>/<div> overlays on first click
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
}})();
"#,
        adblock_on = if adblock_enabled { "true" } else { "false" },
        popup_on = if popup_blocker_enabled {
            "true"
        } else {
            "false"
        },
        blocked_domains_json = blocked_domains_json
    )
}
