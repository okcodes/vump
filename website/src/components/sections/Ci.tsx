import { CI_WORKFLOW } from '../../content/rules.ts';
import { checkRun } from '../../content/runs.ts';
import { Code } from '../ui/Code.tsx';
import { Reveal } from '../ui/Reveal.tsx';
import { Section } from '../ui/Section.tsx';
import { Terminal } from '../ui/Terminal.tsx';

export function Ci() {
  return (
    <Section
      id="ci"
      eyebrow="In your pipeline"
      title="Three lines that stop a bad tag."
      lede="Push a tag and the action checks it against the version in your source. Put it first in the job, and a tag that does not match costs seconds instead of a whole release."
    >
      <div className="grid gap-6 lg:grid-cols-12">
        <Reveal className="lg:col-span-7">
          <Code code={CI_WORKFLOW} lang="yaml" title=".github/workflows/release.yml" />
        </Reveal>

        <Reveal delay={80} className="lg:col-span-5">
          <div className="flex h-full min-w-0 flex-col gap-4">
            <p className="label">When the tag and the source disagree</p>
            <Terminal lines={checkRun} title="ci · verify" animate />
            <div className="border-line bg-raised/40 flex-1 rounded-xl border p-5">
              <p className="text-muted text-sm leading-relaxed">
                The job stops right there. Nothing is compiled, signed or published under a version
                that was never in your source.
              </p>
            </div>
          </div>
        </Reveal>
      </div>
    </Section>
  );
}
