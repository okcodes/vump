import { guidedRun, scriptedRun } from '../../content/runs.ts';
import { Reveal } from '../ui/Reveal.tsx';
import { Section } from '../ui/Section.tsx';
import { Terminal } from '../ui/Terminal.tsx';

export function Rule() {
  return (
    <Section
      id="rule"
      index="02"
      eyebrow="The one rule worth knowing"
      title="Name a subcommand and vump will never prompt."
      lede="Omit one and it will. There is no third behavior, and no configuration that changes this."
    >
      <div className="grid gap-6 lg:grid-cols-2">
        <Reveal>
          <div className="flex h-full min-w-0 flex-col">
            <div className="mb-4 flex items-baseline gap-3">
              <code className="text-ink font-mono text-sm">vump</code>
              <span className="text-faint text-sm">asks, then confirms the whole plan</span>
            </div>
            <Terminal lines={guidedRun} title="guided" animate className="flex-1" />
          </div>
        </Reveal>

        <Reveal delay={80}>
          <div className="flex h-full min-w-0 flex-col">
            <div className="mb-4 flex items-baseline gap-3">
              <code className="text-ink font-mono text-sm">vump patch</code>
              <span className="text-faint text-sm">never asks anything, ever</span>
            </div>
            <Terminal lines={scriptedRun} title="scripted" animate className="flex-1" />
          </div>
        </Reveal>
      </div>

      <Reveal delay={120}>
        <p className="text-muted mt-8 max-w-[70ch] leading-relaxed">
          Anything a subcommand would otherwise have to ask about is a required flag, or an error
          that names what is missing. That is what makes vump safe to put in a script, a CI job, or
          an agent’s toolbelt: a command either has everything it needs, or it fails saying what it
          does not.
        </p>
      </Reveal>
    </Section>
  );
}
