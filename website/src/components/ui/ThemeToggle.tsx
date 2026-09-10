import { useTheme } from '../../lib/theme.ts';
import { MoonIcon, SunIcon } from './icons.tsx';

export function ThemeToggle() {
  const { theme, toggle } = useTheme();

  return (
    <button
      type="button"
      onClick={toggle}
      aria-label={`Switch to the ${theme === 'dark' ? 'light' : 'dark'} theme`}
      className="border-line text-muted hover:border-line-strong hover:text-ink inline-flex h-9 w-9 items-center justify-center rounded-full border transition-colors"
    >
      {theme === 'dark' ? <SunIcon /> : <MoonIcon />}
    </button>
  );
}
