import { TRACKED_FILES } from '../../content/files.ts';
import { links } from '../../content/links.ts';
import { bumpRun } from '../../content/runs.ts';
import { Button } from '../ui/Button.tsx';
import { ArrowIcon } from '../ui/icons.tsx';
import { Reveal } from '../ui/Reveal.tsx';
import { Terminal } from '../ui/Terminal.tsx';

export function Hero() {
  return (
    <section id="top" className="relative overflow-hidden pt-32 md:pt-44">
      <div className="grid-backdrop pointer-events-none absolute inset-0 -z-10" aria-hidden />
      <div
        className="pointer-events-none absolute -top-64 left-1/2 -z-10 h-[38rem] w-[64rem] -translate-x-1/2 rounded-full bg-[radial-gradient(closest-side,var(--glow),transparent)]"
        aria-hidden
      />

      <div className="mx-auto w-full max-w-[78rem] px-6 md:px-10">
        <div className="grid items-center gap-14 lg:grid-cols-12 lg:gap-12">
          <div className="lg:col-span-7">
            <Reveal>
              <p className="label">One binary · macOS, Linux, Windows</p>
            </Reveal>

            <Reveal delay={60}>
              <h1 className="mt-6 text-[clamp(2.5rem,6.2vw,4.5rem)] leading-[0.98] font-medium tracking-[-0.04em]">
                One version.
                <br />
                Every file.
                <br />
                <span className="font-display text-signal italic">One command.</span>
              </h1>
            </Reveal>

            <Reveal delay={120}>
              <p className="text-muted mt-7 max-w-[52ch] text-lg leading-relaxed">
                vump writes your new version into <code className="text-ink">package.json</code>,{' '}
                <code className="text-ink">Cargo.toml</code>,{' '}
                <code className="text-ink">pyproject.toml</code> and their lock files at once — then
                commits, tags, and verifies the tag in CI.
              </p>
            </Reveal>

            <Reveal delay={180}>
              <div className="mt-9 flex flex-wrap items-center gap-3">
                <Button href="#start">Get started</Button>
                <Button href={links.repo} variant="outline" external>
                  View on GitHub
                  <ArrowIcon className="opacity-60" />
                </Button>
              </div>
            </Reveal>
          </div>

          <div className="lg:col-span-5">
            <Reveal delay={140}>
              <Terminal lines={bumpRun} title="~/widget" animate />
            </Reveal>
          </div>
        </div>
      </div>

      <Reveal className="mt-20 md:mt-24">
        <div className="border-line border-t">
          <div className="mx-auto flex w-full max-w-[78rem] flex-col gap-3 px-6 py-5 md:flex-row md:items-center md:gap-8 md:px-10">
            <span className="label shrink-0">Knows these files</span>
            <div className="text-muted flex flex-wrap items-center gap-x-5 gap-y-2 font-mono text-[12.5px]">
              {TRACKED_FILES.map((file) => (
                <span key={file.name} className="whitespace-nowrap">
                  {file.name}
                </span>
              ))}
            </div>
          </div>
        </div>
      </Reveal>
    </section>
  );
}
