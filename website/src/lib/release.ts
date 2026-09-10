import { useEffect, useState } from 'react';

export interface Release {
  version: string;
  url: string;
  prerelease: boolean;
}

interface ApiRelease {
  tag_name: string;
  html_url: string;
  prerelease: boolean;
  draft: boolean;
}

/**
 * The newest published release, read from GitHub when the page loads.
 *
 * This page deliberately keeps no copy of vump's version number. A number
 * written into a marketing site is a number nobody bumps, and a version that
 * disagrees with its source is the exact defect this tool exists to prevent.
 * When the request fails, nothing is shown — an unverifiable version is worse
 * than none.
 */
export function useLatestRelease(): Release | null {
  const [release, setRelease] = useState<Release | null>(null);

  useEffect(() => {
    const controller = new AbortController();

    void (async () => {
      try {
        const response = await fetch(
          'https://api.github.com/repos/okcodes/vump/releases?per_page=5',
          { signal: controller.signal, headers: { Accept: 'application/vnd.github+json' } },
        );
        if (!response.ok) return;

        const releases = (await response.json()) as ApiRelease[];
        const newest = releases.find((entry) => !entry.draft);
        if (!newest) return;

        setRelease({
          version: newest.tag_name.replace(/^v/, ''),
          url: newest.html_url,
          prerelease: newest.prerelease,
        });
      } catch {
        // Offline, rate-limited, or blocked: the chip simply does not appear.
      }
    })();

    return () => controller.abort();
  }, []);

  return release;
}
