# Working in this repository

vump keeps semver version numbers in sync across a repository's files, and
verifies in CI that a released tag matches what is recorded in source. One
crate, ports and adapters.

## Where things are written down

| Document | Holds |
| --- | --- |
| [`DESIGN.md`](DESIGN.md) | How vump behaves and why. Authoritative: code that disagrees with it means one of the two is defective. |
| [`ENGINEERING.md`](ENGINEERING.md) | How code is written and judged here. |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | How work flows: branches, commits, pull requests, releases. |
| [`BACKLOG.md`](BACKLOG.md) | What is not built yet, and what was decided against. |
| [`README.md`](README.md) | The user-facing manual. |

Before changing behavior, read the relevant `DESIGN.md` section. Before
building something new, check `BACKLOG.md` — it may already be decided against,
and the entry says why. Proposing one of those again is fine, but say what
changed.

## The rules that are easiest to get wrong

Each is stated in full, with its reasoning and a worked example, in
[`ENGINEERING.md`](ENGINEERING.md).

- **A type says what a value means.** If a field's meaning depends on which
  caller produced it, the type is wrong. An `Option` is genuine absence, never
  a variant in disguise.
- **Derive what can be derived.** Two stored fields that must agree will
  eventually disagree.
- **Refuse rather than guess.** When vump cannot know, it fails with an
  actionable message. It never picks the likely option.
- **Comments explain why, not what** — and never how the code came to be. No
  development history, no "previously", no narration of a discussion.
- **Tracked files describe the tool, nothing else.** This repository is
  public. Nothing about who works on it, how the work is divided, what tooling
  or hardware sits on a contributor's machine, or how anyone's credentials are
  held belongs in a file that ships with it.
- **A fixed bug arrives with the test that would have caught it.**

## Before pushing

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs exactly this on Linux, macOS and Windows. Running it locally first is
much faster than waiting for a runner to report the same thing.

## Working agreement

- **Open a pull request when a coherent set of work is done, then pause for
  review.** A pull request is a checkpoint, not a formality.
- **Commit often.** Every commit compiles and passes tests on its own; branch
  history is preserved on merge, so each one stays readable forever.
- **Verify locally in preference to CI.** Hosted runners are slow; use them to
  confirm platform differences, not to discover ordinary mistakes.
- **If the same failure repeats, stop and report it.** Iterating on a wedged
  problem costs more than asking.
- **Say plainly what is left for a human to do**, including anything that was
  deliberately left out.
- **Push back.** Honest recommendations are wanted, including for removing
  features or abandoning an approach. Agreement that isn't meant is worth
  nothing here.

## Keeping this current

These files carry the reasoning behind decisions, which is the part that is
expensive to reconstruct. When a decision changes, update the document in the
same pull request as the code — a standard nobody can trace back to its example
stops being persuasive, and stops being followed.
