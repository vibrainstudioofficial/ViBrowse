import { DEFAULT_VERSION, DOWNLOAD_PACKAGES, GITHUB_REPO, type PackageId } from './data';

interface GitHubAsset {
  name: string;
  download_count: number;
  browser_download_url: string;
}

interface GitHubRelease {
  tag_name: string;
  published_at: string;
  assets: GitHubAsset[];
}

export interface DownloadStatsSnapshot {
  source: 'github-live' | 'local-fallback';
  latestVersion: string;
  publishedAt: string | null;
  totalDownloads: number;
  sessionDownloads: number;
  perPackage: Record<PackageId, number>;
  liveUrls: Partial<Record<PackageId, string>>;
  fetchedAt: Date;
}

const SESSION_STORAGE_KEY = 'vibrowse_local_dl_counts_v1';

interface LocalCounts {
  sessionTotal: number;
  perPackage: Record<PackageId, number>;
}

function emptyPerPackage(): Record<PackageId, number> {
  return {
    'win-setup': 0,
    'win-portable': 0,
    'linux-deb': 0,
    'linux-rpm': 0,
  };
}

function loadLocalCounts(): LocalCounts {
  try {
    const raw = localStorage.getItem(SESSION_STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<LocalCounts>;
      return {
        sessionTotal: typeof parsed.sessionTotal === 'number' ? parsed.sessionTotal : 0,
        perPackage: {
          ...emptyPerPackage(),
          ...(parsed.perPackage ?? {}),
        },
      };
    }
  } catch {
    // Ignore storage access errors in private mode
  }
  return { sessionTotal: 0, perPackage: emptyPerPackage() };
}

function saveLocalCounts(counts: LocalCounts): void {
  try {
    localStorage.setItem(SESSION_STORAGE_KEY, JSON.stringify(counts));
  } catch {
    // Ignore storage write errors
  }
}

function classifyAsset(filename: string): PackageId | null {
  const lower = filename.toLowerCase();
  if (lower.endsWith('setup.exe')) return 'win-setup';
  if (lower.endsWith('portable.zip')) return 'win-portable';
  if (lower.endsWith('.deb')) return 'linux-deb';
  if (lower.endsWith('.rpm')) return 'linux-rpm';
  return null;
}

export function recordLocalDownload(pkgId: PackageId): LocalCounts {
  const counts = loadLocalCounts();
  counts.sessionTotal += 1;
  counts.perPackage[pkgId] = (counts.perPackage[pkgId] ?? 0) + 1;
  saveLocalCounts(counts);
  return counts;
}

export function getLocalCounts(): LocalCounts {
  return loadLocalCounts();
}

export async function fetchLiveStats(): Promise<DownloadStatsSnapshot> {
  const local = loadLocalCounts();
  const perPackage = emptyPerPackage();
  const liveUrls: Partial<Record<PackageId, string>> = {};

  try {
    const resp = await fetch(`https://api.github.com/repos/${GITHUB_REPO}/releases`, {
      headers: { Accept: 'application/vnd.github+json' },
    });
    if (!resp.ok) {
      throw new Error(`HTTP ${resp.status}`);
    }
    const releases = (await resp.json()) as GitHubRelease[];
    let totalGithub = 0;

    for (const rel of releases) {
      for (const asset of rel.assets ?? []) {
        totalGithub += asset.download_count ?? 0;
        const pkgId = classifyAsset(asset.name);
        if (pkgId) {
          perPackage[pkgId] += asset.download_count ?? 0;
          if (!liveUrls[pkgId] && asset.browser_download_url) {
            liveUrls[pkgId] = asset.browser_download_url;
          }
        }
      }
    }

    // Add local session clicks so user sees immediate feedback even before GitHub API cache updates
    for (const pkg of DOWNLOAD_PACKAGES) {
      perPackage[pkg.id] += local.perPackage[pkg.id];
    }

    const latest = releases[0];
    return {
      source: 'github-live',
      latestVersion: latest?.tag_name || DEFAULT_VERSION,
      publishedAt: latest?.published_at || null,
      totalDownloads: totalGithub + local.sessionTotal,
      sessionDownloads: local.sessionTotal,
      perPackage,
      liveUrls,
      fetchedAt: new Date(),
    };
  } catch {
    for (const pkg of DOWNLOAD_PACKAGES) {
      perPackage[pkg.id] = local.perPackage[pkg.id];
    }
    return {
      source: 'local-fallback',
      latestVersion: DEFAULT_VERSION,
      publishedAt: null,
      totalDownloads: local.sessionTotal,
      sessionDownloads: local.sessionTotal,
      perPackage,
      liveUrls,
      fetchedAt: new Date(),
    };
  }
}

export function animateCounter(el: HTMLElement, target: number, durationMs = 600): void {
  const current = Number(el.dataset.value ?? '0') || 0;
  el.dataset.value = String(target);
  if (current === target) {
    el.textContent = target.toLocaleString('id-ID');
    return;
  }

  const start = performance.now();
  const step = (now: number) => {
    const progress = Math.min(1, (now - start) / durationMs);
    const eased = 1 - Math.pow(1 - progress, 3);
    const value = Math.round(current + (target - current) * eased);
    el.textContent = value.toLocaleString('id-ID');
    if (progress < 1) {
      requestAnimationFrame(step);
    }
  };
  requestAnimationFrame(step);
}
