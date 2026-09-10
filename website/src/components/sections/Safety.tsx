import { GUARANTEES } from '../../content/rules.ts';
import { Reveal } from '../ui/Reveal.tsx';
import { Section } from '../ui/Section.tsx';

export function Safety() {
  return (
    <Section
      id="safety"
      index="07"
      eyebrow="Safety"
      title="Refuse rather than guess."
      className="bg-sunken/50"
      lede="Where vump cannot know, it stops and names the next action. An error you cannot act on is a defect, and a warning on a path that ends in a published tag is not a safeguard."
    >
      <div className="grid gap-px overflow-hidden md:grid-cols-2 lg:grid-cols-3">
        {GUARANTEES.map((guarantee, index) => (
          <Reveal key={guarantee.title} delay={(index % 3) * 60}>
            <div className="border-line bg-raised/30 h-full border p-6">
              <div className="text-signal mb-4 font-mono text-sm">✓</div>
              <p className="text-ink leading-snug font-medium">{guarantee.title}</p>
              <p className="text-muted mt-3 text-sm leading-relaxed">{guarantee.detail}</p>
            </div>
          </Reveal>
        ))}
      </div>

      <Reveal delay={120}>
        <p className="text-faint mt-8 max-w-[74ch] text-sm leading-relaxed">
          Releases carry keyless build provenance, signed with a short-lived certificate from the
          workflow’s own identity and recorded in a public transparency log. There is no key to
          store, rotate, or leak.
        </p>
      </Reveal>
    </Section>
  );
}
