/**
 * A model for rendered command output.
 *
 * Terminal panes on this page reproduce what vump actually prints — the marks,
 * the column alignment, the wording of its errors. Writing them as segments
 * rather than as pre-formatted strings is what lets the accent land on the part
 * that matters (the version that changed) instead of on the whole line.
 */

export type Tone = 'ink' | 'muted' | 'signal' | 'warn' | 'danger';

export interface Seg {
  text: string;
  tone: Tone;
}

export interface Line {
  /** Prompt lines are typed out; output lines appear a line at a time. */
  prompt: boolean;
  segs: Seg[];
}

const seg =
  (tone: Tone) =>
  (text: string): Seg => ({ text, tone });

/** Ordinary output. */
export const t = seg('ink');
/** Secondary output: paths, hints, anything the eye should pass over. */
export const dim = seg('muted');
/** The accent. Reserved for what changed, and for a passing mark. */
export const sig = seg('signal');
/** A waived guard or a caution. */
export const caution = seg('warn');
/** A refusal or a failing mark. */
export const bad = seg('danger');

/** A line the user types. */
export function cmd(text: string): Line {
  return { prompt: true, segs: [t(text)] };
}

/** A line the program prints. */
export function out(...segs: Array<Seg | string>): Line {
  return {
    prompt: false,
    segs: segs.map((s) => (typeof s === 'string' ? t(s) : s)),
  };
}

/** A blank line. Spacing in real output is part of the output. */
export function gap(): Line {
  return { prompt: false, segs: [] };
}

/** The text of a line, with no styling — what a copy button should yield. */
export function plain(line: Line): string {
  return (line.prompt ? '$ ' : '') + line.segs.map((s) => s.text).join('');
}
