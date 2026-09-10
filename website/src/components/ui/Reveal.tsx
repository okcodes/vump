import type { ReactNode } from 'react';

import { cn } from '../../lib/cn.ts';
import { useInView } from '../../lib/hooks.ts';

interface Props {
  children: ReactNode;
  /** Milliseconds to hold before entering, for staggering a group. */
  delay?: number;
  className?: string;
}

export function Reveal({ children, delay = 0, className }: Props) {
  const { ref, inView } = useInView<HTMLDivElement>();

  return (
    <div
      ref={ref}
      className={cn('reveal min-w-0', className)}
      data-visible={inView}
      style={delay ? { transitionDelay: `${delay}ms` } : undefined}
    >
      {children}
    </div>
  );
}
