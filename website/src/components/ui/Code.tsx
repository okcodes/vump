import { cn } from '../../lib/cn.ts';
import { highlight, type Lang, type TokenKind } from '../../lib/highlight.ts';
import { CopyButton } from './CopyButton.tsx';

const KIND: Record<TokenKind, string> = {
  text: 'text-ink/90',
  comment: 'text-faint italic',
  section: 'text-ink font-medium',
  key: 'text-ink/90',
  value: 'text-signal',
  expr: 'text-warn',
  punct: 'text-faint',
};

interface Props {
  code: string;
  lang: Lang;
  /** The file this snippet belongs in. Shown in the header. */
  title?: string;
  /** Marks lines (1-based) as the ones that matter. */
  emphasis?: number[];
  className?: string;
  copyable?: boolean;
}

export function Code({ code, lang, title, emphasis, className, copyable = true }: Props) {
  const lines = highlight(code, lang);
  const marked = new Set(emphasis ?? []);

  return (
    <div
      className={cn(
        'border-line bg-sunken/60 min-w-0 overflow-hidden rounded-xl border',
        className,
      )}
    >
      {title || copyable ? (
        <div className="border-line flex items-center justify-between gap-4 border-b px-3.5 py-2">
          <span className="label truncate normal-case">{title}</span>
          {copyable ? <CopyButton value={code} /> : null}
        </div>
      ) : null}

      <pre className="overflow-x-auto py-3 font-mono text-[12.5px] leading-[1.8] sm:text-[13px]">
        <code>
          {lines.map((tokens, index) => {
            const highlighted = marked.has(index + 1);
            return (
              <div
                key={index}
                className={cn(
                  'px-4',
                  highlighted && 'bg-signal-soft border-signal border-l-2 pl-[calc(1rem-2px)]',
                )}
              >
                {tokens.length === 0 ? ' ' : null}
                {tokens.map((token, tokenIndex) => (
                  <span key={tokenIndex} className={KIND[token.kind]}>
                    {token.text}
                  </span>
                ))}
              </div>
            );
          })}
        </code>
      </pre>
    </div>
  );
}
