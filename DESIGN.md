# vump — design specification

This document is the authoritative specification for vump's behavior and
architecture. Where the source and this document disagree, treat the
disagreement as a defect in one of them and resolve it deliberately.

## 1. Purpose and scope

vump does two things:

1. **Bump** a semver version number, keeping every file in a repository that
   records that version in sync, and optionally turn the change into a git
   commit, tag, and push.
2. **Check** that a given version — typically a git tag in CI — matches the
   version recorded in source, failing loudly when it does not.

The second job is what makes the first one trustworthy. A tag that disagrees
with the source it claims to describe is a defect that must be caught before
any build, sign, or publish work is spent on it.

vump has no opinion about what a tag triggers downstream. Build matrices,
publishing to registries, and deployment are other tools' problems. vump's
responsibility ends at "the version in source and the tag agree, and both are
correct."

### Non-goals

- Running package-manager commands (`npm install`, `cargo build`, …). vump
  never mutates lock files; it reports when one has likely gone stale.
- Deciding *when* to release, or orchestrating anything after the tag exists.
- Backward compatibility with any earlier configuration or CLI surface.

## 2. Domain model

### Version state machine

A version is either **stable** (`1.2.3`) or a **pre-release**
(`1.2.3-alpha.0`). Pre-release labels are ordered:

```
alpha < beta < rc
```

| Current       | Operation            | Result          |
| ------------- | -------------------- | --------------- |
| `1.2.3`       | `patch`              | `1.2.4`         |
| `1.2.3`       | `minor`              | `1.3.0`         |
| `1.2.3`       | `major`              | `2.0.0`         |
| `1.2.3`       | `alpha --from patch` | `1.2.4-alpha.0` |
| `1.2.3`       | `alpha --from minor` | `1.3.0-alpha.0` |
| `1.2.3-alpha.0` | `alpha`            | `1.2.3-alpha.1` |
| `1.2.3-alpha.2` | `beta`             | `1.2.3-beta.0`  |
| `1.2.3-beta.1`  | `rc`               | `1.2.3-rc.0`    |
| `1.2.3-rc.1`    | `release`          | `1.2.3`         |

Rules that fall out of the table:

- Advancing to the **same** label increments its counter.
- Advancing to a **higher** label resets the counter to `0`.
- Moving to a **lower** label is refused. This is a mistake, not a workflow.
- Starting a pre-release from a stable version requires knowing which stable
  version it precedes, hence the mandatory `--from`.
- `release` on an already-stable version is an error.

### Deliberately excluded: `patch`/`minor`/`major` on a pre-release

Asking for `patch` while on `1.2.3-rc.1` is ambiguous: it could mean "finalize
this as `1.2.3`" or "abandon it and go to `1.2.4`". vump does not guess and
does not ask. It is an error, and the message directs the user to `release`
first, then bump from the resulting stable version. Two explicit steps beat one
command with a hidden fork.

### Version files

A version file is any file that records the project's version. Detection is by
filename:

| Filename       | Format     | Location of the version   |
| -------------- | ---------- | ------------------------- |
| `package.json` | JSON       | `.version`                |
| `Cargo.toml`   | TOML       | `[package].version`       |
| `VERSION`      | plain text | entire file contents      |

**Writes must preserve the rest of the file byte-for-byte.** Update the version
field in place rather than parsing and re-serializing: key order, indentation,
whitespace, and comments elsewhere in the file must survive untouched. A tool
that reformats a `package.json` as a side effect of bumping it will not be
tolerated in a repository, and correctness here is more important than the
elegance of the parsing approach.

Support for arbitrary formats via a declarative extraction spec (a path or
pattern per file entry) is a plausible future direction, but the three built-in
formats come first.

## 3. Configuration

A single `vump.toml`, discovered by walking upward from the working directory
to the nearest one — the same way git locates `.git`. TOML is the only
supported format.

> **Rationale for TOML over YAML/JSON.** YAML's implicit typing is actively
> hazardous for a tool whose entire purpose is exact version strings: `1.0`
> becomes a float and loses its trailing zero, and bare `yes`/`no` become
> booleans. JSON has no comments, which the annotated default config depends
> on. Supporting several formats at once would also mean the same tool looks
> different in every repository, which is precisely the confusion this design
> avoids elsewhere.

### Single-project repository

The common case carries no ceremony:

```toml
[[files]]
path = "Cargo.toml"
```

### Multi-project repository

Independently-versioned projects in one repository are named, and addressed by
name:

```toml
[[project]]
name = "api"
files = ["services/api/Cargo.toml"]

[[project]]
name = "web"
files = ["apps/web/package.json"]
```

```sh
vump patch --project api
```

Naming rather than locating projects matters because the caller is frequently
not sitting in the project's directory — CI jobs and external automation invoke
vump from the repository root or from nowhere in particular. A model based on
per-directory config files cannot express "bump the project called `api`"
without first knowing where `api` lives.

`vump status` with no project selected reports every declared project and
whether its files agree.

### Git integration

```toml
[git]
commit = true
commit_message = "chore: bump version to v{new_version}"
tag = true
tag_pattern = "v{new_version}"
push = false
```

**Configuration is authoritative.** If `vump.toml` says `commit = true`, vump
commits — it does not ask, in any mode. Settings present in configuration are
decisions the user has already made; re-asking them is a defect. Command-line
flags override configuration for a single invocation.

## 4. CLI contract

The single most important property of this CLI: **whether an invocation is
interactive is determined by exactly one thing — the presence of a
subcommand — and there are no exceptions.**

| Invocation                   | Behavior                                     |
| ---------------------------- | -------------------------------------------- |
| `vump`                       | Interactive. Always.                          |
| `vump patch` (and siblings)  | Non-interactive. Never prompts, under any circumstances. |

A user must never have to wonder whether a command is about to block on a
prompt. An invocation that would need information it does not have **fails with
an actionable error** rather than falling back to asking:

- **Files disagree** on the current version → error listing the disagreement.
  There is no flag to resolve this non-interactively; a source of truth
  contradicting itself is an exceptional state that a human should look at.
- **Starting a pre-release from stable without `--from`** → error. `--from` is
  required, not optional-with-a-prompt.
- **`patch`/`minor`/`major` on a pre-release** → error, per §2.

In interactive mode, the only question vump asks unconditionally is the bump
type, and its menu lists **only operations valid from the current version** —
invalid choices are omitted, not displayed with an explanation of why they will
fail. Everything else is asked only when configuration has not already settled
it.

There is deliberately **no `-y`/`--auto-approve` flag.** Once a subcommand
never prompts by construction, the subcommand *is* the confirmation, in the
same way `rm file` needs no confirmation flag.

### Commands

```
vump                      Interactive bump
vump patch|minor|major    Bump a stable version
vump alpha|beta|rc        Start or advance a pre-release (--from required from stable)
vump release              Drop the pre-release suffix
vump check <version>      Verify tracked files match the given version
vump status               Report current versions and sync state
vump init                 Create a vump.toml
vump update               Replace the running binary with the latest release
```

### Flags

| Flag                | Applies to      | Description                                    |
| ------------------- | --------------- | ---------------------------------------------- |
| `--dry-run`         | bump commands   | Compute and report the plan, write nothing     |
| `--from <bump>`     | pre-release     | Stable bump the pre-release is based on        |
| `--project <name>`  | all             | Select a project in a multi-project repository |
| `--commit`          | bump commands   | Stage and commit the changed files             |
| `--tag`             | bump commands   | Commit and tag (implies `--commit`)            |
| `--push`            | bump commands   | Push commit and tag (implies `--commit`)       |
| `--no-git`          | bump commands   | Perform no git actions, overriding configuration |
| `--json`            | global          | Machine-readable output                        |

`--no-git` is the single escape hatch from authoritative configuration, and it
conflicts with the three flags above: asking for no git actions and for a
commit in one invocation is a contradiction, rejected at parse time.

Only files declared in configuration are staged. If commit, tag, or push is
active, a dirty working tree is a hard failure — a version bump must not sweep
unrelated changes into its commit.

If push fails after a successful commit or tag, vump reports exactly what did
succeed and prints the command to finish manually. Partial success must never
be reported as total failure.

## 5. Output and exit codes

Human-readable output is the default. `--json` selects machine-readable output
and is a presentation concern only: both renderings are produced from the same
structured result, and neither may carry information the other lacks.

`--json` requires a non-interactive invocation. Combined with bare `vump`, it
is an error rather than an attempt to interleave prompts with parseable output.

JSON output covers every command, not only `check`. For bump commands it
reports each file changed with its old and new version, the commit created, the
tag name, and whether the push succeeded — values that come into existence
during the run and that a caller would otherwise have to parse out of prose.

Decorative output (colour, symbols) is suppressed automatically when stdout is
not a terminal.

Exit codes are part of the public contract. Each distinct failure mode gets its
own stable code so callers can branch without matching strings:

| Code | Meaning                                        |
| ---- | ---------------------------------------------- |
| 0    | Success                                        |
| 1    | Generic / unexpected error                     |
| 2    | Usage error (bad arguments)                    |
| 3    | Configuration missing or invalid               |
| 4    | Version mismatch (`check` failed)              |
| 5    | Tracked files disagree with each other         |
| 6    | Working tree dirty                             |
| 7    | Invalid version transition                     |
| 8    | Git operation failed                           |

## 6. Architecture

Ports and adapters, sized to the problem: one crate, clear internal
boundaries. Splitting into multiple crates is premature until something is
demonstrably reusable outside this binary.

```
src/
  domain/     Pure logic. No I/O, no clock, no environment.
              Version state machine, bump planning, sync analysis.
  ports/      Traits describing what the domain needs from the world.
              VersionFile, Vcs, ConfigSource, Interaction, ReleaseSource.
  adapters/   Concrete implementations of the ports.
              Filesystem version files, git via subprocess, TOML config,
              terminal prompts, GitHub releases.
  app/        Use cases wiring ports together: Bump, Check, Status, Init, Update.
              This is the layer worth testing hardest.
  cli/        clap definitions, adapter selection, output rendering.
```

Rules that keep the boundaries real:

- `domain` depends on nothing but the standard library and pure helpers. If it
  needs to read a file or know the time, the design is wrong.
- `app` depends on `domain` and `ports`, never on `adapters`. Use cases are
  constructed with port implementations supplied by `cli`.
- A `BumpPlan` value produced by `domain` is the single input to both the
  human renderer, the JSON renderer, and `--dry-run`. Those three must not
  compute anything themselves.

### Adapter notes

- **git** is invoked as a subprocess rather than through a library. This
  inherits the user's real git configuration, credential helpers, hooks, and
  SSH setup for free, and keeps the dependency surface small.
- **Interaction** has two implementations: a terminal one, and one that returns
  an error for every question. Non-interactive mode is enforced by construction
  — it is given the erroring implementation, so a prompt cannot leak into a
  scripted run even by accident.
- **Self-update** replaces the running binary. On Unix an atomic rename works
  even while executing; on Windows the running executable must be renamed aside
  first, then replaced. That platform difference is delegated to a library
  rather than reimplemented. Downgrades are refused: a development build ahead
  of the last release must never be overwritten by it.
- **Release discovery** uses the endpoint that excludes pre-releases, so an
  alpha never presents itself as an upgrade to someone on a stable build. This
  pairs with the release workflow publishing pre-release tags as pre-releases.

### Dependency notes

Prefer `rustls` over any OpenSSL-backed TLS stack for anything performing
network I/O: release binaries are statically linked against musl, and native
TLS does not cooperate with that.

## 7. Testing

Three layers, each with a distinct job:

1. **Domain unit tests.** Every transition in the §2 table, plus the refused
   ones. Pure functions, no fixtures, exhaustive.
2. **Use-case tests** against in-memory port implementations. A fake VCS and a
   fake filesystem let the whole bump flow be exercised — including git
   side-effects and failure paths — without a real repository.
3. **End-to-end tests** driving the compiled binary against temporary
   directories, asserting on stdout, stderr, and exit codes. This layer owns
   the §4 interactivity contract and the §5 exit-code table: it is the only
   place that can prove a subcommand never prompts.

Formatting preservation deserves explicit tests: bump a file with unusual
indentation, key order, and comments, and assert that only the version bytes
changed.

## 8. Release engineering

Release assets are named `vump-<os>-<arch>` (with `.exe` on Windows), plus
`vump-darwin-universal`. These names are a contract: the CI check action and
in-place self-update both resolve them.

- Linux binaries link statically against musl so they run on any distribution
  regardless of glibc version.
- macOS ships a single universal binary, which is the only macOS artifact
  signed and notarized. Bare Mach-O executables cannot be stapled; Gatekeeper
  verifies online on first launch.
- The release workflow verifies the tag against source *before* building
  anything.
