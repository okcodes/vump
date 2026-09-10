import type { ReactNode } from 'react';

import { cn } from '../../lib/cn.ts';
import { Reveal } from './Reveal.tsx';

interface Props {
  id: string;
  eyebrow: string;
  title: ReactNode;
  lede?: ReactNode;
  children: ReactNode;
  className?: string;
}

export function Section({ id, eyebrow, title, lede, children, className }: Props) {
  return (
    <section id={id} className={cn('border-line scroll-mt-24 border-t py-20 md:py-24', className)}>
      <div className="mx-auto w-full max-w-[78rem] px-6 md:px-10">
        <Reveal>
          <p className="label text-signal">{eyebrow}</p>

          <h2 className="mt-5 max-w-[22ch] text-3xl leading-[1.08] font-medium tracking-[-0.03em] text-balance md:text-[2.6rem]">
            {title}
          </h2>

          {lede ? (
            <p className="text-muted mt-4 max-w-[58ch] text-base leading-relaxed md:text-[1.0625rem]">
              {lede}
            </p>
          ) : null}
        </Reveal>

        <div className="mt-10 md:mt-12">{children}</div>
      </div>
    </section>
  );
}
