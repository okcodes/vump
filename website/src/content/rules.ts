export interface Transition {
  from: string;
  command: string;
  result: string;
}

/** The version state machine, in the order the manual states it. */
export const TRANSITIONS: Transition[] = [
  { from: '1.2.3', command: 'patch', result: '1.2.4' },
  { from: '1.2.3', command: 'minor', result: '1.3.0' },
  { from: '1.2.3', command: 'major', result: '2.0.0' },
  { from: '1.2.3', command: 'alpha --toward minor', result: '1.3.0-alpha.0' },
  { from: '1.2.3-alpha.0', command: 'alpha', result: '1.2.3-alpha.1' },
  { from: '1.2.3-alpha.2', command: 'beta', result: '1.2.3-beta.0' },
  { from: '1.2.3-rc.1', command: 'release', result: '1.2.3' },
];

export interface Refusal {
  title: string;
  reason: string;
}

export const REFUSALS: Refusal[] = [
  {
    title: 'Moving to a less mature channel',
    reason:
      'rc back to beta. There is no flag to force it, because there is no workflow that wants it.',
  },
  {
    title: 'patch, minor or major while on a pre-release',
    reason:
      'It is ambiguous between finalizing and abandoning. Run release first, then bump from there.',
  },
  {
    title: 'A pre-release from a stable version',
    reason:
      'Without --toward, nothing says which release it precedes. The flag is required, not guessed at.',
  },
];

export interface Guarantee {
  title: string;
  detail: string;
}

export const GUARANTEES: Guarantee[] = [
  {
    title: 'A dirty tree stops the run',
    detail:
      'Checked before anything is written, not after — so a refused release leaves the working tree exactly as it found it.',
  },
  {
    title: 'Only declared files are staged',
    detail:
      'Unrelated work in the tree cannot ride along in a version-bump commit, however it got there.',
  },
  {
    title: 'Nothing is half-applied',
    detail:
      'Everything knowable before writing is checked before writing, so a run that cannot finish cleanly does nothing at all.',
  },
  {
    title: 'Partial success is reported exactly',
    detail:
      'A push that fails after the commit and tag exits 8 and prints the command that finishes the job by hand.',
  },
  {
    title: 'Downloads are verified',
    detail:
      'Every release publishes SHA256SUMS, checked before a binary is written to disk or run. A release publishing none is refused, not warned about.',
  },
  {
    title: 'No package manager is ever run',
    detail:
      'No dependency resolution, no registry, no install scripts. An in-place edit that needs neither a network nor a dependency graph.',
  },
];

export interface ExitCode {
  code: number;
  meaning: string;
}

export const EXIT_CODES: ExitCode[] = [
  { code: 0, meaning: 'Success' },
  { code: 1, meaning: 'Unexpected failure, a declined prompt, or an update is available' },
  { code: 2, meaning: 'Usage error' },
  { code: 3, meaning: 'Configuration missing or invalid' },
  { code: 4, meaning: 'Version mismatch — check failed' },
  { code: 5, meaning: 'Tracked files disagree with each other' },
  { code: 6, meaning: 'Working tree dirty' },
  { code: 7, meaning: 'Invalid version transition' },
  { code: 8, meaning: 'Git operation failed' },
  { code: 9, meaning: 'A release artifact could not be trusted' },
  { code: 10, meaning: 'A release could not be obtained' },
];

export const CI_WORKFLOW = `on:
  push:
    tags: ['v*']

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      # First, so a tag that lies costs nothing.
      - uses: okcodes/vump/.github/actions/check@main
        with:
          version: \${{ github.ref_name }}

      - run: cargo build --release`;

export const MULTI_PROJECT_CONFIG = `[git]
tag_pattern = "{project}-v{new_version}"

[[project]]
name = "api"
files = ["services/api/Cargo.toml", "Cargo.lock"]

[[project]]
name = "web"
files = ["apps/web/package.json"]`;

export const CONFIG = `files = ["VERSION", "ui/package.json"]

[git]
through = "tag"                  # none, commit, tag, or push
commit_message = "chore: bump version to v{new_version}"
tag_pattern = "v{new_version}"
release_branches = ["main"]`;
