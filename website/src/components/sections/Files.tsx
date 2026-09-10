import { useRef, useState, type KeyboardEvent } from 'react';

import { TRACKED_FILES } from '../../content/files.ts';
import { repairRun } from '../../content/runs.ts';
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
      index="03"
      eyebrow="What it writes"
      title="Nine files. One number. Nothing else touched."
      lede="Files are recognized by name, and a rewrite changes the version and nothing more — key order, indentation, and comments elsewhere in the file survive untouched. Pick one to see exactly which line moves."
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
            className="flex h-full min-w-0 flex-col gap-4"
          >
            <Code code={file.code} lang={file.lang} title={file.name} emphasis={file.emphasis} />

            <div className="border-line grid gap-4 rounded-xl border p-5 sm:grid-cols-2">
              <div>
                <p className="label">Version lives in</p>
                <p className="text-ink mt-2 font-mono text-[13px] leading-relaxed">{file.where}</p>
              </div>
              <div>
                <p className="label">Never mistaken for it</p>
                <p className="text-muted mt-2 text-sm leading-relaxed">{file.guard}</p>
              </div>
            </div>

            <p className="text-faint text-xs leading-relaxed">
              <code className="font-mono">.fsproj</code> and{' '}
              <code className="font-mono">.vbproj</code> are read exactly like{' '}
              <code className="font-mono">.csproj</code>, and{' '}
              <code className="font-mono">Directory.Build.targets</code> like{' '}
              <code className="font-mono">Directory.Build.props</code> — except that a targets file
              overrides a project’s own version, where a props file only supplies a default.
            </p>
          </div>
        </Reveal>
      </div>

      <div className="mt-16 grid gap-8 lg:grid-cols-12 lg:gap-10">
        <Reveal className="lg:col-span-6">
          <h3 className="text-xl font-medium tracking-[-0.02em]">
            Lock files move with the manifest
          </h3>
          <p className="text-muted mt-4 leading-relaxed">
            <code className="text-ink font-mono text-[13px]">Cargo.lock</code> and{' '}
            <code className="text-ink font-mono text-[13px]">package-lock.json</code> record their
            project’s own version, and{' '}
            <code className="text-ink font-mono text-[13px]">cargo build --locked</code> and{' '}
            <code className="text-ink font-mono text-[13px]">npm ci</code> both reject a tree where
            a lock and its manifest disagree. vump writes them in the same run, so nothing is left
            to finish afterwards. If a lock file records your version but is missing from the
            configuration, it stops before writing anything and names it.
          </p>
          <p className="text-faint mt-4 text-sm leading-relaxed">
            This is not vump running a package manager, which it never does. The test is whether the
            result can be computed with no network and no knowledge of the dependency graph.
          </p>
        </Reveal>

        <Reveal delay={80} className="lg:col-span-6">
          <h3 className="text-xl font-medium tracking-[-0.02em]">And when they disagree</h3>
          <p className="text-muted mt-4 mb-5 leading-relaxed">
            A bump requires the tracked files to agree, and refuses when they do not — a source of
            truth contradicting itself is something to look at, not to guess about.
          </p>
          <Terminal lines={repairRun} title="repair" animate />
        </Reveal>
      </div>
    </Section>
  );
}
