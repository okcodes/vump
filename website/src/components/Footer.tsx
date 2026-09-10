import { links } from '../content/links.ts';
import { Mark } from './ui/icons.tsx';

const COLUMNS = [
  {
    title: 'Project',
    items: [
      { label: 'Repository', href: links.repo },
      { label: 'Releases', href: links.releases },
      { label: 'CI action', href: links.action },
      { label: 'Sandbox projects', href: links.sandbox },
    ],
  },
  {
    title: 'Documentation',
    items: [
      { label: 'Manual', href: links.readme },
      { label: 'Design specification', href: links.design },
      { label: 'Contributing', href: links.contributing },
      { label: 'Backlog', href: links.backlog },
    ],
  },
];

export function Footer() {
  return (
    <footer className="border-line border-t py-16">
      <div className="mx-auto w-full max-w-[78rem] px-6 md:px-10">
        <div className="grid gap-12 md:grid-cols-[1.4fr_1fr_1fr]">
          <div>
            <div className="text-ink flex items-center gap-2.5">
              <Mark className="h-6 w-6" />
              <span className="text-[15px] font-semibold tracking-[-0.02em]">vump</span>
            </div>
            <p className="text-muted mt-4 max-w-[38ch] text-sm leading-relaxed">
              Keep semver version numbers in sync across the files of a repository, and verify in CI
              that a released tag matches what is recorded in source.
            </p>
          </div>

          {COLUMNS.map((column) => (
            <div key={column.title}>
              <p className="label">{column.title}</p>
              <ul className="mt-4 space-y-2.5">
                {column.items.map((item) => (
                  <li key={item.label}>
                    <a
                      href={item.href}
                      target="_blank"
                      rel="noreferrer noopener"
                      className="text-muted hover:text-ink text-sm transition-colors"
                    >
                      {item.label}
                    </a>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>

        <div className="border-line text-faint mt-14 flex flex-col gap-3 border-t pt-6 font-mono text-[11.5px] sm:flex-row sm:items-center sm:justify-between">
          <p>The documentation lives with the code, where it stays true.</p>
          <p>This page keeps no copy of vump’s version number.</p>
        </div>
      </div>
    </footer>
  );
}
