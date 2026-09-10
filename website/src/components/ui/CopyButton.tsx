import { useCopy } from '../../lib/hooks.ts';
import { cn } from '../../lib/cn.ts';
import { CheckIcon, CopyIcon } from './icons.tsx';

interface Props {
  value: string;
  label?: string;
  className?: string;
  tone?: 'terminal' | 'page';
}

export function CopyButton({ value, label = 'Copy', className, tone = 'page' }: Props) {
  const { copied, copy } = useCopy(value);

  return (
    <button
      type="button"
      onClick={copy}
      aria-label={copied ? 'Copied' : `${label} to clipboard`}
      className={cn(
        'group inline-flex items-center gap-1.5 rounded-md px-2 py-1 font-mono text-[11px] tracking-wide transition-colors',
        tone === 'terminal'
          ? 'text-term-muted hover:text-term-ink hover:bg-white/5'
          : 'text-faint hover:bg-ink/5 hover:text-ink',
        className,
      )}
    >
      {copied ? (
        <CheckIcon className="text-signal" />
      ) : (
        <CopyIcon className="opacity-70 transition-opacity group-hover:opacity-100" />
      )}
      <span aria-hidden>{copied ? 'Copied' : label}</span>
    </button>
  );
}
