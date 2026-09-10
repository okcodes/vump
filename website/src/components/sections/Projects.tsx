import { MULTI_PROJECT_CONFIG } from '../../content/rules.ts';
import { statusRun } from '../../content/runs.ts';
import { Code } from '../ui/Code.tsx';
import { Reveal } from '../ui/Reveal.tsx';
import { Section } from '../ui/Section.tsx';
import { Terminal } from '../ui/Terminal.tsx';

export function Projects() {
  return (
    <Section
      id="projects"
      index="06"
      eyebrow="Monorepos"
      title="Independently-versioned projects, addressed by name."
      lede="Replace the file list with named projects and each one moves on its own. Naming rather than locating them is deliberate: the caller is frequently not sitting in the project’s directory."
    >
      <div className="grid gap-6 lg:grid-cols-12">
        <Reveal className="lg:col-span-6">
          <Code code={MULTI_PROJECT_CONFIG} lang="toml" title="vump.toml" />
        </Reveal>

        <Reveal delay={80} className="lg:col-span-6">
          <Terminal lines={statusRun} title="~/monorepo" animate />
        </Reveal>
      </div>

      <div className="mt-14 grid gap-8 lg:grid-cols-2 lg:gap-12">
        <Reveal>
          <h3 className="text-xl font-medium tracking-[-0.02em]">Tags that name their project</h3>
          <p className="text-muted mt-4 leading-relaxed">
            Projects moving independently need distinguishable tags, or they collide the moment two
            reach the same version and nothing can say which project a pushed tag describes. One{' '}
            <code className="text-ink font-mono text-[13px]">tag_pattern</code> covers every
            project, or each overrides its own. If two would produce the same tag, vump says so
            rather than guessing — and CI can then pass a pushed tag straight through without
            knowing which project it names.
          </p>
        </Reveal>

        <Reveal delay={80}>
          <h3 className="text-xl font-medium tracking-[-0.02em]">One vump.toml per repository</h3>
          <p className="text-muted mt-4 leading-relaxed">
            A second configuration deeper in the tree is what{' '}
            <code className="text-ink font-mono text-[13px]">[[project]]</code> exists to prevent:
            scattered configurations cannot be addressed by name, git operations still run against
            the enclosing repository, and a pushed tag can no longer be traced back to what produced
            it. vump refuses that rather than warning about it — a warning arrives too late to help
            once a tag exists.
          </p>
        </Reveal>
      </div>
    </Section>
  );
}
