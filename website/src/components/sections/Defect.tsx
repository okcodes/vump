import { checkFailRun } from '../../content/runs.ts';
import { Reveal } from '../ui/Reveal.tsx';
import { Section } from '../ui/Section.tsx';
import { Terminal } from '../ui/Terminal.tsx';

const SPEC = [
  { key: 'Exit code', value: '4' },
  { key: 'Reported by', value: 'vump check' },
  { key: 'Spent so far', value: 'nothing' },
];

export function Defect() {
  return (
    <Section
      id="defect"
      index="01"
      eyebrow="Why it exists"
      title={
        <>
          A tag that disagrees with its source is a{' '}
          <span className="font-display text-signal italic">defect</span>.
        </>
      }
      lede="A version number lives in more than one file, and files drift. The build that notices is usually the one that has already compiled, signed and published under a number that was never true — and the repair costs a deleted tag and a redone release."
    >
      <div className="grid gap-6 lg:grid-cols-12">
        <Reveal className="lg:col-span-7">
          <Terminal lines={checkFailRun} title="ci · verify" animate />
        </Reveal>

        <Reveal delay={80} className="lg:col-span-5">
          <div className="border-line bg-raised/40 flex h-full flex-col rounded-xl border p-6">
            <p className="text-ink text-[1.0625rem] leading-relaxed">
              Put the check first in the workflow and a bad tag is rejected in seconds, before a
              runner has spent a minute on it.
            </p>

            <dl className="border-line mt-6 border-t">
              {SPEC.map((row) => (
                <div key={row.key} className="border-line flex justify-between border-b py-3">
                  <dt className="label normal-case">{row.key}</dt>
                  <dd className="text-signal font-mono text-[13px]">{row.value}</dd>
                </div>
              ))}
            </dl>

            <p className="text-muted mt-6 text-sm leading-relaxed">
              Every failure mode has its own exit code, so a script branches on the number instead
              of matching strings against an error message.
            </p>
          </div>
        </Reveal>
      </div>
    </Section>
  );
}
