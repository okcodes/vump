import { useEffect, useState } from 'react';

import { cn } from '../../lib/cn.ts';
import { useInView, useReducedMotion } from '../../lib/hooks.ts';
import { plain, type Line, type Tone } from '../../lib/terminal.ts';
import { CopyButton } from './CopyButton.tsx';

const TONES: Record<Tone, string> = {
  ink: 'text-term-ink',
  muted: 'text-term-muted',
  signal: 'text-term-signal',
  warn: 'text-term-warn',
  danger: 'text-term-danger',
};

const CHAR_MS = 24;
const AFTER_COMMAND_MS = 320;
const BETWEEN_LINES_MS = 80;

interface Props {
  lines: Line[];
  /** Shown in the pane's header. A path, or the shell being used. */
  title?: string;
  /** Types the run out on first sight. Off for panes that are pure reference. */
  animate?: boolean;
  /** Overrides what the copy button yields. Defaults to the commands only. */
  copy?: string;
  className?: string;
}

/**
 * A pane of command output.
 *
 * Unrevealed text is rendered transparent rather than omitted, so the pane is
 * its final size from the first frame and the page never reflows around it
 * while a run plays.
 */
export function Terminal({ lines, title, animate = false, copy, className }: Props) {
  const reduced = useReducedMotion();
  const { ref, inView } = useInView<HTMLDivElement>('0px 0px -15% 0px');
  const playing = animate && !reduced && inView;
  const [typed, setTyped] = useState(() => ({ line: 0, char: 0 }));
  const cursor = playing ? typed : { line: lines.length, char: 0 };

  useEffect(() => {
    if (!playing) return;

    let timer = 0;
    let stopped = false;

    const step = (line: number, char: number) => {
      if (stopped || line >= lines.length) return;
      const target = lines[line];
      if (!target) return;

      const width = target.segs.reduce((total, s) => total + s.text.length, 0);
      const typing = target.prompt && char < width;
      const delay = typing ? CHAR_MS : target.prompt ? AFTER_COMMAND_MS : BETWEEN_LINES_MS;
      const next = typing ? { line, char: char + 1 } : { line: line + 1, char: 0 };

      timer = window.setTimeout(() => {
        setTyped(next);
        step(next.line, next.char);
      }, delay);
    };

    step(0, 0);

    return () => {
      stopped = true;
      window.clearTimeout(timer);
    };
  }, [lines, playing]);

  const copyValue =
    copy ??
    lines
      .filter((line) => line.prompt)
      .map((line) => plain(line).replace(/^\$ /, ''))
      .join('\n');

  return (
    <div
      ref={ref}
      className={cn(
        'bg-term-bg min-w-0 overflow-hidden rounded-xl border border-white/10',
        'shadow-[0_30px_80px_-40px_rgba(0,0,0,0.85)]',
        className,
      )}
    >
      <div className="border-term-line flex items-center justify-between gap-4 border-b px-3.5 py-2">
        <span className="text-term-muted truncate font-mono text-[11px] tracking-[0.12em]">
          {title ?? 'zsh'}
        </span>
        {copyValue ? <CopyButton value={copyValue} tone="terminal" /> : null}
      </div>

      <pre className="text-term-ink overflow-x-auto px-4 py-3.5 font-mono text-[12.5px] leading-[1.75] sm:text-[13px]">
        <code>
          {lines.map((line, index) => {
            const revealed = index < cursor.line;
            const active = index === cursor.line;
            let budget = active ? cursor.char : revealed ? Number.POSITIVE_INFINITY : 0;

            return (
              <div key={index} className="min-h-[1.75em]">
                {line.prompt ? (
                  <span className={revealed || active ? 'text-term-signal' : 'opacity-0'}>$ </span>
                ) : null}
                {line.segs.map((seg, segIndex) => {
                  const shown = Math.max(0, Math.min(seg.text.length, budget));
                  budget -= seg.text.length;
                  return (
                    <span key={segIndex} className={TONES[seg.tone]}>
                      {seg.text.slice(0, shown)}
                      <span className="opacity-0">{seg.text.slice(shown)}</span>
                    </span>
                  );
                })}
                {playing && active && line.prompt ? <span className="caret" aria-hidden /> : null}
              </div>
            );
          })}
        </code>
      </pre>
    </div>
  );
}
