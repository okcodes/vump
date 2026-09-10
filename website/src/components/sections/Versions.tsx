import { REFUSALS, TRANSITIONS } from '../../content/rules.ts';
import { Reveal } from '../ui/Reveal.tsx';
import { Section } from '../ui/Section.tsx';

export function Versions() {
  return (
    <Section
      id="versions"
      index="04"
      eyebrow="Version rules"
      title="Every transition is a rule, not a guess."
      lede="Pre-release channels are ordered alpha, then beta, then rc. Each command has exactly one result from any given version — and where the intent would be ambiguous, vump refuses instead of choosing."
    >
      <Reveal>
        <div className="border-line overflow-hidden rounded-xl border">
          <div className="border-line bg-raised/40 grid grid-cols-[1fr_1.2fr_1fr] gap-4 border-b px-5 py-3">
            <span className="label">Current</span>
            <span className="label">Command</span>
            <span className="label">Result</span>
          </div>
          {TRANSITIONS.map((row) => (
            <div
              key={`${row.from}-${row.command}`}
              className="border-line grid grid-cols-[1fr_1.2fr_1fr] items-center gap-4 border-b px-5 py-3.5 font-mono text-[13px] last:border-b-0"
            >
              <span className="text-muted">{row.from}</span>
              <span className="text-ink">
                <span className="text-faint">vump </span>
                {row.command}
              </span>
              <span className="text-signal">{row.result}</span>
            </div>
          ))}
        </div>
      </Reveal>

      <div className="mt-14">
        <Reveal>
          <div className="flex items-baseline gap-4">
            <span className="label text-warn">Refused, deliberately</span>
            <span className="bg-line h-px flex-1" aria-hidden />
          </div>
        </Reveal>

        <div className="mt-6 grid gap-5 md:grid-cols-3">
          {REFUSALS.map((refusal, index) => (
            <Reveal key={refusal.title} delay={index * 60}>
              <div className="border-line bg-raised/40 h-full rounded-xl border p-5">
                <div className="text-warn mb-3 font-mono text-sm">✗</div>
                <p className="text-ink leading-snug font-medium">{refusal.title}</p>
                <p className="text-muted mt-3 text-sm leading-relaxed">{refusal.reason}</p>
              </div>
            </Reveal>
          ))}
        </div>
      </div>
    </Section>
  );
}
