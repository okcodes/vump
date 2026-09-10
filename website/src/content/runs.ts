/**
 * The command output shown on the page.
 *
 * These reproduce what the binary actually prints — the marks, the column
 * alignment, the wording. Showing friendlier output than the tool produces
 * would be advertising something that does not exist.
 */

import { bad, cmd, dim, gap, out, sig, t, type Line } from '../lib/terminal.ts';

/** A patch bump carried through to a tag. */
export const bumpRun: Line[] = [
  cmd('vump patch --through tag'),
  out(sig('✓ '), t('0.2.0 -> '), sig('0.2.1')),
  out(dim('  Cargo.toml')),
  out(dim('  Cargo.lock')),
  out(sig('✓ '), t('committed  '), dim('chore: bump version to v0.2.1')),
  out(sig('✓ '), t('tagged     '), sig('v0.2.1')),
];

/** A tag that does not describe the source it claims to. */
export const checkRun: Line[] = [
  cmd('vump check v1.4.0'),
  out(bad('✗ '), t('version mismatch: expected 1.4.0')),
  gap(),
  out(bad('  ✗  '), t('Cargo.toml    '), bad('1.3.2')),
  out(sig('  ✓  '), t('package.json  '), t('1.4.0')),
];

/** Adopting a repository: init finds the version files and writes them down. */
export const initRun: Line[] = [
  cmd('vump init'),
  out(sig('✓ '), t('wrote vump.toml tracking:')),
  out(dim('  Cargo.toml')),
  out(dim('  Cargo.lock')),
];

/** Every project in the repository, at a glance. */
export const statusRun: Line[] = [
  cmd('vump status'),
  out(sig('✓  '), t('api  '), sig('1.4.2')),
  out(sig('✓  '), t('web  '), sig('0.9.0')),
  gap(),
  cmd('vump patch --project api'),
  out(sig('✓ '), t('1.4.2 -> '), sig('1.4.3')),
];
