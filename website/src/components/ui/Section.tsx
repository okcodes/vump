import type { ReactNode } from 'react';

import { cn } from '../../lib/cn.ts';
import { Reveal } from './Reveal.tsx';

interface Props {
  id: string;
  /** The section's number, in the instrument's own voice. */
  index: string;
  eyebrow: string;
  title: ReactNode;
  lede?: ReactNode;
  children: ReactNode;
  className?: string;
}

export function Section({ id, index, eyebrow, title, lede, children, className }: Props) {
  return (
    <section id={id} className={cn('border-line scroll-mt-24 border-t py-20 md:py-28', className)}>
      <div className="mx-auto w-full max-w-[78rem] px-6 md:px-10">
        <Reveal>
          <div className="flex items-baseline gap-4">
            <span className="label text-signal">{index}</span>
            <span className="label">{eyebrow}</span>
          </div>

          <h2 className="mt-6 max-w-[24ch] text-3xl leading-[1.08] font-medium tracking-[-0.03em] text-balance md:text-[2.9rem]">
            {title}
          </h2>

          {lede ? (
            <p className="text-muted mt-5 max-w-[62ch] text-base leading-relaxed md:text-[1.0625rem]">
              {lede}
            </p>
          ) : null}
        </Reveal>

        <div className="mt-12 md:mt-16">{children}</div>
      </div>
    </section>
  );
}
