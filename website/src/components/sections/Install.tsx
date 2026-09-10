import { links } from '../../content/links.ts';
import { initRun, selfRun } from '../../content/runs.ts';
import { Button } from '../ui/Button.tsx';
import { Code } from '../ui/Code.tsx';
import { ArrowIcon } from '../ui/icons.tsx';
import { Reveal } from '../ui/Reveal.tsx';
import { Terminal } from '../ui/Terminal.tsx';

const SOURCE = `git clone https://github.com/okcodes/vump
cd vump
cargo install --path .`;

export function Install() {
  return (
    <section id="install" className="border-line relative overflow-hidden border-t py-20 md:py-28">
      <div className="grid-backdrop pointer-events-none absolute inset-0 -z-10" aria-hidden />
      <div
        className="pointer-events-none absolute -bottom-72 left-1/2 -z-10 h-[36rem] w-[60rem] -translate-x-1/2 rounded-full bg-[radial-gradient(closest-side,var(--glow),transparent)]"
        aria-hidden
      />

      <div className="mx-auto w-full max-w-[78rem] px-6 md:px-10">
        <Reveal>
          <div className="flex items-baseline gap-4">
            <span className="label text-signal">08</span>
            <span className="label">Install</span>
          </div>
          <h2 className="mt-6 max-w-[20ch] text-3xl leading-[1.08] font-medium tracking-[-0.03em] md:text-[2.9rem]">
            Two commands from here.
          </h2>
          <p className="text-muted mt-5 max-w-[58ch] text-base leading-relaxed md:text-[1.0625rem]">
            <code className="text-ink font-mono text-[14px]">vump init</code> writes a configuration
            tracking the version files it finds. After that, a bump is one word.
          </p>
        </Reveal>

        <div className="mt-12 grid gap-6 lg:grid-cols-3">
          <Reveal>
            <div className="flex h-full min-w-0 flex-col gap-4">
              <p className="label">Download a binary</p>
              <div className="border-line bg-raised/40 flex flex-1 flex-col rounded-xl border p-5">
                <p className="text-muted flex-1 text-sm leading-relaxed">
                  A signed universal binary for macOS, static musl builds for Linux, and Windows
                  including arm64. Every release publishes checksums, and both the installer and the
                  CI action verify against them.
                </p>
                <Button
                  href={links.releases}
                  variant="outline"
                  external
                  className="mt-6 self-start"
                >
                  Releases
                  <ArrowIcon className="opacity-60" />
                </Button>
              </div>
            </div>
          </Reveal>

          <Reveal delay={60}>
            <div className="flex h-full min-w-0 flex-col gap-4">
              <p className="label">Build from source</p>
              <Code code={SOURCE} lang="bash" copyable />
            </div>
          </Reveal>

          <Reveal delay={120}>
            <div className="flex h-full min-w-0 flex-col gap-4">
              <p className="label">Already have it</p>
              <Terminal lines={selfRun} title="self" animate />
            </div>
          </Reveal>
        </div>

        <Reveal delay={80}>
          <div className="mt-10 grid gap-6 lg:grid-cols-2">
            <Terminal lines={initRun} title="~/your-repo" animate />
            <p className="text-muted self-center text-sm leading-relaxed">
              <code className="text-ink font-mono text-[13px]">vump self update</code> keeps the
              binary current, and <code className="text-ink font-mono text-[13px]">--channel</code>{' '}
              names the least mature release you will accept — a floor rather than an exact match,
              so tracking release candidates never silently moves you onto the next minor’s first
              alpha.
            </p>
          </div>
        </Reveal>
      </div>
    </section>
  );
}
