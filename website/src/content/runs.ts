/**
 * Every pane on this page is what the binary prints.
 *
 * The marks, the column widths, the wording of the errors and the blank lines
 * between them are copied from vump's own renderer rather than invented for the
 * screenshot. A site that shows friendlier output than the tool produces is
 * advertising something that does not exist.
 */

import { bad, caution, cmd, dim, gap, out, sig, t, type Line } from '../lib/terminal.ts';

/** A patch bump carried through to a tag. */
export const bumpRun: Line[] = [
  cmd('vump patch --through tag'),
  out(sig('✓ '), t('0.2.0 -> '), sig('0.2.1')),
  out(dim('  Cargo.toml')),
  out(dim('  Cargo.lock')),
  out(sig('✓ '), t('committed  '), dim('chore: bump version to v0.2.1')),
  out(sig('✓ '), t('tagged     '), sig('v0.2.1')),
  gap(),
  out(dim('To push:')),
  out(dim('  git push && git push origin v0.2.1')),
];

/** A tag that does not describe the source it claims to. */
export const checkFailRun: Line[] = [
  cmd('vump check v1.4.0'),
  out(bad('✗ '), t('version mismatch: expected 1.4.0')),
  gap(),
  out(bad('  ✗  '), t('Cargo.toml    '), bad('1.3.2')),
  out(sig('  ✓  '), t('package.json  '), t('1.4.0')),
];

/** The same check, on a tag that tells the truth. */
export const checkPassRun: Line[] = [
  cmd('vump check v1.4.0'),
  out(sig('✓ '), t('2 files match 1.4.0')),
];

/** Omitting a subcommand: vump asks, then shows the whole plan before acting. */
export const guidedRun: Line[] = [
  cmd('vump'),
  out(caution('? '), t('Current version 1.2.3. Bump to:  '), sig('patch  ->  1.2.4')),
  out(caution('? '), t('Carry the release through:  '), sig('Tag        — commit and tag')),
  gap(),
  out(dim('  Bumping:  '), t('1.2.3  ->  '), sig('1.2.4')),
  out(dim('  File:     '), t('Cargo.toml')),
  out(dim('  File:     '), t('Cargo.lock')),
  out(dim('  Commit:   '), t('chore: bump version to v1.2.4')),
  out(dim('  Tag:      '), t('v1.2.4')),
  out(dim('  Push:     '), t('no')),
  gap(),
  out(caution('? '), t('Proceed? '), sig('Yes')),
  out(sig('✓ '), t('1.2.3 -> '), sig('1.2.4')),
];

/** Naming one: no questions, and a missing decision is an error instead. */
export const scriptedRun: Line[] = [
  cmd('vump patch --through tag'),
  out(sig('✓ '), t('1.2.3 -> '), sig('1.2.4')),
  out(dim('  Cargo.toml')),
  out(sig('✓ '), t('committed  '), dim('chore: bump version to v1.2.4')),
  out(sig('✓ '), t('tagged     '), sig('v1.2.4')),
  gap(),
  cmd('vump alpha'),
  out(bad('error: '), t('1.2.3 is stable, so `alpha` needs to know which')),
  out(t('release it leads to; pass --toward patch, --toward minor,')),
  out(t('or --toward major')),
];

/** Files that have drifted apart, and the one command that repairs them. */
export const repairRun: Line[] = [
  cmd('vump patch'),
  out(bad('error: '), t('tracked files disagree about the current version:')),
  out(t('  VERSION       '), bad('1.2.3')),
  out(t('  package.json  '), bad('0.9.0')),
  gap(),
  cmd('vump set 2.0.0'),
  out(sig('✓ '), t('set to '), sig('2.0.0')),
  out(dim('  VERSION')),
  out(dim('  package.json')),
];

/** Every project in the repository, at a glance. */
export const statusRun: Line[] = [
  cmd('vump status'),
  out(sig('✓  '), t('api  '), sig('1.4.2')),
  out(sig('✓  '), t('web  '), sig('0.9.0')),
  gap(),
  cmd('vump minor --project api'),
  out(sig('✓ '), t('1.4.2 -> '), sig('1.5.0')),
  out(dim('  services/api/Cargo.toml')),
  out(dim('  Cargo.lock')),
];

/** Keeping the binary current. */
export const selfRun: Line[] = [
  cmd('vump self status'),
  out(t('0.8.0 is available; running 0.7.0.')),
  out(dim('Run `vump self update` to install it.')),
  gap(),
  cmd('vump self update'),
  out(sig('✓ '), t('updated 0.7.0 -> '), sig('0.8.0')),
];

/** Adopting a repository: init finds the version files and writes them down. */
export const initRun: Line[] = [
  cmd('vump init'),
  out(sig('✓ '), t('wrote vump.toml tracking:')),
  out(dim('  Cargo.toml')),
  out(dim('  Cargo.lock')),
  gap(),
  out(dim('Review it, then run `vump` to bump.')),
];
