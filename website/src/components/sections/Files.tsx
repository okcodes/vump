import { useRef, useState, type KeyboardEvent } from 'react';

import { TRACKED_FILES } from '../../content/files.ts';
import { MULTI_PROJECT_CONFIG } from '../../content/rules.ts';
import { statusRun } from '../../content/runs.ts';
import { cn } from '../../lib/cn.ts';
import { Code } from '../ui/Code.tsx';
import { Reveal } from '../ui/Reveal.tsx';
import { Section } from '../ui/Section.tsx';
import { Terminal } from '../ui/Terminal.tsx';

export function Files() {
  const [selected, setSelected] = useState(0);
  const tabs = useRef<Array<HTMLButtonElement | null>>([]);

  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    const step = event.key === 'ArrowDown' || event.key === 'ArrowRight' ? 1 : 0;
    const back = event.key === 'ArrowUp' || event.key === 'ArrowLeft' ? -1 : 0;
    if (!step && !back) return;

    event.preventDefault();
    const next = (selected + step + back + TRACKED_FILES.length) % TRACKED_FILES.length;
    setSelected(next);
    tabs.current[next]?.focus();
  };

  const file = TRACKED_FILES[selected] ?? TRACKED_FILES[0];
  if (!file) return null;

  return (
    <Section
      id="files"
      eyebrow="Works with"
      title="Your files, changed one line at a time."
      lede="Files are recognized by name — no configuration beyond listing them. Only the version moves: formatting, key order and comments stay exactly where they were."
    >
      <div className="grid gap-6 lg:grid-cols-12">
        <Reveal className="lg:col-span-4">
          <div
            role="tablist"
            aria-label="Supported files"
            aria-orientation="vertical"
            className="border-line bg-raised/40 flex gap-1 overflow-x-auto rounded-xl border p-1.5 lg:flex-col lg:overflow-visible"
          >
            {TRACKED_FILES.map((entry, index) => (
              <button
                key={entry.name}
                type="button"
                role="tab"
                id={`file-tab-${entry.name}`}
                aria-selected={index === selected}
                aria-controls="file-panel"
                tabIndex={index === selected ? 0 : -1}
                ref={(node) => {
                  tabs.current[index] = node;
                }}
                onClick={() => setSelected(index)}
                onKeyDown={onKeyDown}
                className={cn(
                  'flex shrink-0 items-baseline justify-between gap-4 rounded-lg px-3.5 py-2.5 text-left transition-colors lg:shrink',
                  index === selected
                    ? 'bg-signal-soft text-ink'
                    : 'text-muted hover:bg-ink/[0.04] hover:text-ink',
                )}
              >
                <span className="font-mono text-[13px] whitespace-nowrap">{entry.name}</span>
                <span
                  className={cn(
                    'hidden font-mono text-[11px] lg:block',
                    index === selected ? 'text-signal' : 'text-faint',
                  )}
                >
                  {entry.ecosystem}
                </span>
              </button>
            ))}
          </div>
        </Reveal>

        <Reveal delay={60} className="lg:col-span-8">
          <div
            id="file-panel"
            role="tabpanel"
            aria-labelledby={`file-tab-${file.name}`}
            className="h-full"
          >
            <Code
              code={file.code}
              lang={file.lang}
              title={file.name}
              emphasis={file.emphasis}
              className="h-full"
            />
          </div>
        </Reveal>
      </div>

      <Reveal delay={80}>
        <div className="border-line mt-14 grid items-center gap-8 rounded-xl border p-6 lg:grid-cols-12 lg:gap-10 lg:p-8">
          <div className="lg:col-span-5">
            <h3 className="text-xl font-medium tracking-[-0.02em]">
              Monorepos, project by project
            </h3>
            <p className="text-muted mt-3 leading-relaxed">
              Name each project in <code className="text-ink font-mono text-[13px]">vump.toml</code>{' '}
              and they version independently, from anywhere in the repository.
            </p>
            <div className="mt-5">
              <Code code={MULTI_PROJECT_CONFIG} lang="toml" title="vump.toml" />
            </div>
          </div>

          <div className="min-w-0 lg:col-span-7">
            <Terminal lines={statusRun} title="~/monorepo" animate />
          </div>
        </div>
      </Reveal>
    </Section>
  );
}
