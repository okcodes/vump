import { links } from '../content/links.ts';
import { Mark } from './ui/icons.tsx';

const LINKS = [
  { label: 'Documentation', href: links.readme },
  { label: 'Releases', href: links.releases },
  { label: 'GitHub', href: links.repo },
];

export function Footer() {
  return (
    <footer className="border-line border-t py-12">
      <div className="mx-auto flex w-full max-w-[78rem] flex-col gap-6 px-6 md:flex-row md:items-center md:justify-between md:px-10">
        <div className="flex items-center gap-3">
          <Mark className="h-6 w-6" />
          <span className="text-[15px] font-semibold tracking-[-0.02em]">vump</span>
          <span className="text-faint hidden text-sm sm:inline">
            Semver versions, kept in sync.
          </span>
        </div>

        <nav aria-label="Footer" className="flex flex-wrap items-center gap-x-7 gap-y-2">
          {LINKS.map((link) => (
            <a
              key={link.label}
              href={link.href}
              target="_blank"
              rel="noreferrer noopener"
              className="text-muted hover:text-ink text-sm transition-colors"
            >
              {link.label}
            </a>
          ))}
        </nav>
      </div>
    </footer>
  );
}
