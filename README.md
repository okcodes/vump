# vump

Keep semver version numbers in sync across the files of a repository, and
verify in CI that a released tag matches what is recorded in source.

A tag that disagrees with the source it claims to describe is a defect. vump
exists to make that impossible to ship: it moves every version in lockstep, and
it fails a build before any expensive work is spent on a tag that lies.

## Install

Download a binary from [Releases](https://github.com/okcodes/vump/releases),
or build from source:

```bash
cargo install --path .
```

## Quick start

```bash
vump init      # writes a vump.toml tracking the version files it finds
vump           # guided bump
```

## The one rule worth knowing

**Naming a subcommand means vump will never prompt. Omitting one means it
will.** There is no third behavior, and no configuration that changes this.

```bash
vump               # guided: asks what to bump, then confirms
vump patch         # never asks anything, ever
```

Anything a subcommand would otherwise have to ask about is a required flag or
an error with an actionable message. This is what makes vump safe to put in a
script, a CI job, or an agent's toolbelt: a command either has everything it
needs, or it fails saying what is missing.

## Commands

| Command | Description |
| --- | --- |
| `vump` | Guided bump |
| `vump patch\|minor\|major` | Bump a stable version |
| `vump alpha\|beta\|rc` | Start or advance a pre-release |
| `vump release` | Drop the pre-release suffix |
| `vump check <version>` | Verify tracked files record this version |
| `vump status` | Report recorded versions and whether they agree |
| `vump init` | Create a `vump.toml` |

## Flags

| Flag | Description |
| --- | --- |
| `--dry-run` | Report what would change, write nothing |
| `--from <patch\|minor\|major>` | Which release a new pre-release leads to |
| `--commit` | Stage and commit the changed files |
| `--tag` | Commit and tag (implies `--commit`) |
| `--push` | Push the commit and tag (implies `--commit`) |
| `--no-git` | Do no git actions, overriding `vump.toml` for this run |
| `--project <name>` | Select a project in a multi-project repository |
| `--json` | Machine-readable output |

## Configuration

`vump.toml`, found by searching upward from the working directory, so vump need
not be run from the repository root.

```toml
files = ["VERSION", "ui/package.json"]

[git]
commit = true
tag = true
push = false
commit_message = "chore: bump version to v{new_version}"
tag_pattern = "v{new_version}"
```

**Configuration is authoritative.** A setting present here is a decision
already made: vump acts on it without asking again, in guided runs too. Flags
add to it for a single run; `--no-git` opts out for a single run.

### Independently-versioned projects

Replace `files` with named projects, and address them by name:

```toml
[[project]]
name = "api"
files = ["services/api/Cargo.toml"]

[[project]]
name = "web"
files = ["apps/web/package.json"]
```

```bash
vump patch --project api
vump status                 # every project at a glance
```

Naming rather than locating projects is deliberate: the caller is frequently
not sitting in the project's directory.

## Supported files

Recognized by filename. Rewrites change the version and nothing else — key
order, indentation, and comments elsewhere in the file survive untouched.

| Filename | Version location |
| --- | --- |
| `package.json` | top-level `version` |
| `Cargo.toml` | `[package].version` |
| `VERSION` | the whole file |

A `version` nested under `dependencies` is never mistaken for the project's
own.

## Version rules

Pre-release channels are ordered `alpha < beta < rc`.

| Current | Command | Result |
| --- | --- | --- |
| `1.2.3` | `patch` | `1.2.4` |
| `1.2.3` | `minor` | `1.3.0` |
| `1.2.3` | `major` | `2.0.0` |
| `1.2.3` | `alpha --from minor` | `1.3.0-alpha.0` |
| `1.2.3-alpha.0` | `alpha` | `1.2.3-alpha.1` |
| `1.2.3-alpha.2` | `beta` | `1.2.3-beta.0` |
| `1.2.3-rc.1` | `release` | `1.2.3` |

Refused, deliberately:

- **Moving to a less mature channel** (`rc` → `beta`). There is no flag to
  force it, because there is no workflow that wants it.
- **`patch`/`minor`/`major` while on a pre-release.** It is ambiguous between
  finalizing and abandoning. Run `release` first, then bump.
- **A pre-release from a stable version without `--from`.** A pre-release must
  know which release it precedes.

## Verifying a tag in CI

The composite action downloads vump and checks the tag against source. Put it
first, so a bad tag costs nothing:

```yaml
- uses: okcodes/vump/.github/actions/check@main
  with:
    version: ${{ github.ref_name }}
```

| Input | Required | Default | Description |
| --- | --- | --- | --- |
| `version` | yes | — | Version to verify (`1.2.3` or `v1.2.3`) |
| `config` | no | `vump.toml` | Path to `vump.toml` from the repo root |
| `vump-version` | no | `latest` | Release to download |

## Scripting and automation

`--json` renders every command's result as structured output, including bump
results and errors. Both renderings come from the same result, so neither
carries information the other lacks.

```bash
vump check "$TAG" --json
vump patch --tag --json
```

Exit codes are a stable contract:

| Code | Meaning |
| --- | --- |
| 0 | Success |
| 1 | Unexpected failure, or a guided run declined |
| 2 | Usage error |
| 3 | Configuration missing or invalid |
| 4 | Version mismatch (`check` failed) |
| 5 | Tracked files disagree with each other |
| 6 | Working tree dirty |
| 7 | Invalid version transition |
| 8 | Git operation failed |

A push that fails after a successful commit and tag exits 8 and prints the
command to finish by hand — partial success is never reported as total failure.

## Safety

- A dirty working tree stops a run that would commit, before anything is
  written.
- Only files declared in configuration are staged, so unrelated work cannot
  ride along in a version-bump commit.
- vump never runs a package manager. It changes versions; refreshing lock files
  is yours to do.

## Development

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

[`DESIGN.md`](DESIGN.md) records the intended behavior and architecture, and is
the authority when the code and your expectations disagree.
