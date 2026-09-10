/**
 * A small, deliberately incomplete highlighter.
 *
 * The page shows a handful of short TOML, YAML, JSON and shell snippets, all
 * written here. A parser for each language, or a library carrying them, would be
 * several times the size of everything it ever renders. Colour is restricted to
 * three roles — structure, value, aside — so a snippet reads the same way the
 * terminal panes do.
 */

export type Lang = 'toml' | 'yaml' | 'json' | 'bash' | 'xml';

export type TokenKind = 'text' | 'comment' | 'section' | 'key' | 'value' | 'expr' | 'punct';

export interface Token {
  text: string;
  kind: TokenKind;
}

const PATTERNS: Record<Lang, RegExp> = {
  // Strings come first everywhere, so a `#` inside one is never a comment.
  toml: /("[^"]*")|(#.*$)|(^\s*\[\[?[^\]\n]+\]\]?)|([A-Za-z_][\w.-]*)(?=\s*=)|(\b\d[\w.+-]*)|([[\]{}=,])/gm,
  yaml: /("[^"]*"|'[^']*')|(#.*$)|(\$\{\{[^}]*\}\})|(^\s*-?\s*[A-Za-z_][\w.-]*)(?=\s*:)|(\b\d[\w.+-]*)|([[\]{}:,-])/gm,
  json: /("(?:[^"\\]|\\.)*")(?=\s*:)|("(?:[^"\\]|\\.)*")|(\b\d[\w.+-]*)|([[\]{},:])/gm,
  bash: /("[^"]*"|'[^']*')|(#.*$)|(\s--?[A-Za-z][\w-]*)|(^\s*[a-z][\w.-]*)|([|&<>$])/gm,
  xml: /("[^"]*")|(<!--[\s\S]*?-->)|(<\/?[A-Za-z][\w.:-]*)|([A-Za-z][\w.:-]*)(?==)|(\/?>)|(\b\d[\w.+-]*)/gm,
};

const KINDS: Record<Lang, TokenKind[]> = {
  toml: ['value', 'comment', 'section', 'key', 'value', 'punct'],
  yaml: ['value', 'comment', 'expr', 'key', 'value', 'punct'],
  json: ['key', 'value', 'value', 'punct'],
  bash: ['value', 'comment', 'punct', 'key', 'punct'],
  xml: ['value', 'comment', 'section', 'key', 'punct', 'value'],
};

/** Splits a snippet into lines of tokens. */
export function highlight(code: string, lang: Lang): Token[][] {
  const pattern = PATTERNS[lang];
  const kinds = KINDS[lang];

  return code.split('\n').map((line) => {
    const tokens: Token[] = [];
    let last = 0;

    pattern.lastIndex = 0;
    for (let match = pattern.exec(line); match; match = pattern.exec(line)) {
      const group = match.findIndex((value, index) => index > 0 && value !== undefined);
      if (group < 1) continue;

      if (match.index > last) {
        tokens.push({ text: line.slice(last, match.index), kind: 'text' });
      }
      tokens.push({ text: match[group] ?? '', kind: kinds[group - 1] ?? 'text' });
      last = match.index + (match[group]?.length ?? 0);
    }

    if (last < line.length) tokens.push({ text: line.slice(last), kind: 'text' });
    return tokens;
  });
}
