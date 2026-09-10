import { useCallback, useEffect, useState } from 'react';

export type Theme = 'dark' | 'light';

const KEY = 'vump-theme';

function current(): Theme {
  return document.documentElement.dataset['theme'] === 'light' ? 'light' : 'dark';
}

/**
 * Reads the theme the pre-paint script in `index.html` already resolved, and
 * writes any change back to it. The stored value is a deliberate choice, so it
 * outranks the system preference from then on.
 */
export function useTheme() {
  const [theme, setTheme] = useState<Theme>(current);

  useEffect(() => {
    document.documentElement.dataset['theme'] = theme;
    try {
      localStorage.setItem(KEY, theme);
    } catch {
      // A browser refusing storage is not a reason to refuse the theme.
    }
  }, [theme]);

  const toggle = useCallback(() => setTheme((t) => (t === 'dark' ? 'light' : 'dark')), []);

  return { theme, toggle };
}
