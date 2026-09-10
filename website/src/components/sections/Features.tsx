import { Reveal } from '../ui/Reveal.tsx';
import { Section } from '../ui/Section.tsx';

const FEATURES = [
  {
    title: 'Every file at once',
    body: 'One command writes the new version into every file that records it, lock files included. Nothing is left half-bumped, and nothing is left to finish by hand.',
    command: 'vump minor',
  },
  {
    title: 'Guided, or completely silent',
    body: 'Run vump on its own and it asks what to bump, then shows you the plan before it touches anything. Name the bump and it asks nothing at all — which is what makes it safe in a script.',
    command: 'vump patch',
  },
  {
    title: 'Commit, tag, push',
    body: 'Carry the bump as far as you want in the same run. Or write the files only, and handle git yourself.',
    command: 'vump patch --through push',
  },
];

export function Features() {
  return (
    <Section
      id="features"
      eyebrow="What it does"
      title="Releases that never miss a file."
      lede="A version number usually lives in more than one place. vump moves all of them together, so the tag you push describes the code you built."
    >
      <div className="grid gap-5 md:grid-cols-3">
        {FEATURES.map((feature, index) => (
          <Reveal key={feature.title} delay={index * 70}>
            <div className="border-line bg-raised/40 flex h-full flex-col rounded-xl border p-6">
              <h3 className="text-lg font-medium tracking-[-0.02em]">{feature.title}</h3>
              <p className="text-muted mt-3 flex-1 text-[15px] leading-relaxed">{feature.body}</p>
              <code className="text-signal border-line mt-6 border-t pt-4 font-mono text-[13px]">
                {feature.command}
              </code>
            </div>
          </Reveal>
        ))}
      </div>
    </Section>
  );
}
