export interface ReleaseAsset {
  name: string;
  browser_download_url: string;
  size: number;
}

export interface GitHubRelease {
  tag_name: string;
  name: string;
  html_url: string;
  published_at: string;
  body: string;
  prerelease: boolean;
  assets: ReleaseAsset[];
}

/**
 * 2つのセマンティックバージョン文字列を比較する
 * @returns v1 > v2 のとき 1, v1 < v2 のとき -1, 等しいとき 0
 */
export function compareSemver(v1: string, v2: string): number {
  const clean1 = v1.replace(/^v/i, "").trim();
  const clean2 = v2.replace(/^v/i, "").trim();

  // プレリリース表記 (-beta, -alpha 等) の分離
  const [main1] = clean1.split("-");
  const [main2] = clean2.split("-");

  const parts1 = main1.split(".").map((p) => parseInt(p, 10) || 0);
  const parts2 = main2.split(".").map((p) => parseInt(p, 10) || 0);

  const maxLen = Math.max(parts1.length, parts2.length);
  for (let i = 0; i < maxLen; i++) {
    const num1 = parts1[i] ?? 0;
    const num2 = parts2[i] ?? 0;
    if (num1 > num2) return 1;
    if (num1 < num2) return -1;
  }

  return 0;
}

/**
 * 新しいバージョンが利用可能かどうかを判定する
 */
export function isNewVersionAvailable(currentVersion: string, latestTag: string): boolean {
  return compareSemver(latestTag, currentVersion) > 0;
}

let cachedRelease: { release: GitHubRelease; timestamp: number } | null = null;
const CACHE_TTL_MS = 5 * 60 * 1000; // 5分間キャッシュ

/**
 * GitHub Releases API から最新のリリース情報を取得する
 */
export async function fetchLatestRelease(forceRefresh = false): Promise<GitHubRelease | null> {
  const now = Date.now();
  if (!forceRefresh && cachedRelease && now - cachedRelease.timestamp < CACHE_TTL_MS) {
    return cachedRelease.release;
  }

  try {
    const response = await fetch("https://api.github.com/repos/blue1st/kindle-glean/releases/latest", {
      headers: {
        Accept: "application/vnd.github.v3+json",
      },
    });

    if (!response.ok) {
      console.warn(`GitHub API request failed with status: ${response.status}`);
      return null;
    }

    const data: GitHubRelease = await response.json();
    cachedRelease = {
      release: data,
      timestamp: now,
    };
    return data;
  } catch (error) {
    console.error("Failed to fetch latest release from GitHub:", error);
    return null;
  }
}
