/** Inline icons, drawn to one grid: 16px box, 1.6 stroke, round caps. */

type Props = { className?: string };

const base = {
  viewBox: '0 0 16 16',
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 1.6,
  strokeLinecap: 'round' as const,
  strokeLinejoin: 'round' as const,
  'aria-hidden': true,
};

export function CopyIcon({ className }: Props) {
  return (
    <svg {...base} className={className} width="16" height="16">
      <rect x="5.5" y="5.5" width="8" height="8" rx="1.8" />
      <path d="M10.5 3.5A1.5 1.5 0 0 0 9 2H4a2 2 0 0 0-2 2v5a1.5 1.5 0 0 0 1.5 1.5" />
    </svg>
  );
}

export function CheckIcon({ className }: Props) {
  return (
    <svg {...base} className={className} width="16" height="16">
      <path d="m3 8.5 3.2 3.2L13 5" />
    </svg>
  );
}

export function ArrowIcon({ className }: Props) {
  return (
    <svg {...base} className={className} width="16" height="16">
      <path d="M4.5 11.5 11.5 4.5M6 4.5h5.5V10" />
    </svg>
  );
}

export function GitHubIcon({ className }: Props) {
  return (
    <svg
      viewBox="0 0 16 16"
      fill="currentColor"
      aria-hidden
      className={className}
      width="16"
      height="16"
    >
      <path d="M8 .2a8 8 0 0 0-2.53 15.6c.4.07.55-.18.55-.39l-.01-1.38c-2.23.48-2.7-1.07-2.7-1.07-.36-.93-.89-1.18-.89-1.18-.73-.5.05-.49.05-.49.8.06 1.23.83 1.23.83.72 1.23 1.88.88 2.34.67.07-.52.28-.88.5-1.08-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82a7.6 7.6 0 0 1 4 0c1.53-1.03 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.28.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48l-.01 2.2c0 .21.15.46.55.38A8 8 0 0 0 8 .2Z" />
    </svg>
  );
}

export function SunIcon({ className }: Props) {
  return (
    <svg {...base} className={className} width="16" height="16">
      <circle cx="8" cy="8" r="3" />
      <path d="M8 1.5v1.2M8 13.3v1.2M14.5 8h-1.2M2.7 8H1.5M12.6 3.4l-.85.85M4.25 11.75l-.85.85M12.6 12.6l-.85-.85M4.25 4.25l-.85-.85" />
    </svg>
  );
}

export function MoonIcon({ className }: Props) {
  return (
    <svg {...base} className={className} width="16" height="16">
      <path d="M13.5 9.6A5.8 5.8 0 0 1 6.4 2.5a5.8 5.8 0 1 0 7.1 7.1Z" />
    </svg>
  );
}

/**
 * The mark: a bump over a baseline. It is the favicon at 16px too, which is why
 * it is two strokes and nothing else.
 */
export function Mark({ className }: Props) {
  return (
    <svg viewBox="0 0 32 32" aria-hidden className={className} width="24" height="24">
      <rect width="32" height="32" rx="7" className="fill-ink/[0.06] stroke-line" strokeWidth="1" />
      <path
        d="M9.5 17.5 16 11l6.5 6.5"
        fill="none"
        stroke="var(--signal)"
        strokeWidth="2.6"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="M9.5 22.5h13"
        fill="none"
        stroke="currentColor"
        strokeOpacity="0.35"
        strokeWidth="2.2"
        strokeLinecap="round"
      />
    </svg>
  );
}
