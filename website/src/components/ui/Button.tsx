import type { ReactNode } from 'react';

import { cn } from '../../lib/cn.ts';

interface Props {
  href: string;
  children: ReactNode;
  variant?: 'solid' | 'outline' | 'ghost';
  external?: boolean;
  className?: string;
}

const VARIANTS = {
  solid:
    'bg-signal text-signal-ink hover:brightness-[1.08] shadow-[0_10px_30px_-12px_var(--signal)]',
  outline: 'border border-line-strong text-ink hover:bg-ink/[0.04]',
  ghost: 'text-muted hover:text-ink',
} as const;

export function Button({ href, children, variant = 'solid', external = false, className }: Props) {
  return (
    <a
      href={href}
      {...(external ? { target: '_blank', rel: 'noreferrer noopener' } : {})}
      className={cn(
        'inline-flex h-11 items-center justify-center gap-2 rounded-full px-5 text-[0.9375rem] font-medium transition-all duration-200',
        VARIANTS[variant],
        className,
      )}
    >
      {children}
    </a>
  );
}
