import { links } from '../../content/links.ts';
import { CI_WORKFLOW, EXIT_CODES } from '../../content/rules.ts';
import { checkPassRun } from '../../content/runs.ts';
import { Code } from '../ui/Code.tsx';
import { ArrowIcon } from '../ui/icons.tsx';
import { Reveal } from '../ui/Reveal.tsx';
import { Section } from '../ui/Section.tsx';
import { Terminal } from '../ui/Terminal.tsx';

export function Ci() {
  return (
    <Section
      id="ci"
      index="05"
      eyebrow="In CI, and in scripts"
      title="Prove the tag before you spend anything on it."
      lede="A composite action downloads vump and checks the pushed tag against the version recorded in source. It is three lines, and it belongs at the top of the job."
    >
      <div className="grid gap-6 lg:grid-cols-12">
        <Reveal className="lg:col-span-7">
          <Code code={CI_WORKFLOW} lang="yaml" title=".github/workflows/release.yml" />
        </Reveal>

        <Reveal delay={80} className="lg:col-span-5">
          <div className="flex h-full min-w-0 flex-col gap-6">
            <Terminal lines={checkPassRun} title="ci · verify" animate />
            <div className="border-line bg-raised/40 rounded-xl border p-5">
              <p className="text-muted text-sm leading-relaxed">
                The action resolves the release binary by name and verifies it against the release’s
                published <code className="text-ink font-mono text-[12.5px]">SHA256SUMS</code>{' '}
                before running it.
              </p>
              <a
                href={links.action}
                target="_blank"
                rel="noreferrer noopener"
                className="text-ink hover:text-signal mt-4 inline-flex items-center gap-1.5 text-sm transition-colors"
              >
                Inputs and defaults
                <ArrowIcon className="opacity-60" />
              </a>
            </div>
          </div>
        </Reveal>
      </div>

      <div className="mt-16">
        <Reveal>
          <div className="flex items-baseline gap-4">
            <span className="label">Exit codes are a stable contract</span>
            <span className="bg-line h-px flex-1" aria-hidden />
          </div>
          <p className="text-muted mt-5 max-w-[62ch] text-sm leading-relaxed">
            Each distinct failure has its own code, so a caller branches on a number rather than
            matching strings. <code className="text-ink font-mono text-[12.5px]">--json</code>{' '}
            renders every command’s result — bumps and errors alike — from the same structured value
            the human output comes from, so neither can carry information the other lacks.
          </p>
        </Reveal>

        <Reveal delay={60}>
          <div className="border-line mt-7 grid gap-x-10 rounded-xl border p-2 sm:grid-cols-2">
            {EXIT_CODES.map((entry) => (
              <div
                key={entry.code}
                className="border-line flex items-baseline gap-4 border-b px-3 py-2.5 last:border-b-0 sm:[&:nth-last-child(2)]:border-b-0"
              >
                <span
                  className={
                    entry.code === 0
                      ? 'text-signal w-6 shrink-0 font-mono text-[13px]'
                      : 'text-warn w-6 shrink-0 font-mono text-[13px]'
                  }
                >
                  {entry.code}
                </span>
                <span className="text-muted text-sm leading-snug">{entry.meaning}</span>
              </div>
            ))}
          </div>
        </Reveal>
      </div>
    </Section>
  );
}
