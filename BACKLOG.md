# Backlog

Work that is not built yet, and decisions taken against building things.

[`DESIGN.md`](DESIGN.md) describes how vump behaves *now* and is authoritative
for the code. This file is the opposite: it holds what is not settled. When an
item here is built, its rules move into `DESIGN.md` and the entry is deleted.

**Decided against** entries exist so a question is not reopened without new
information. If you find yourself proposing one of them, say what changed.

An entry is ready to build when its *Problem* is agreed. The *Shape* is a
starting point, not a specification — expect it to change while building.

---

## Ready to build

Ranked by value.

### 1. Set an exact version

**Problem.** There is no way to say "make everything 1.4.0". Files that
disagree are a dead end in non-interactive mode: vump reports the
disagreement and stops, and the only repair is editing by hand.

**Why it matters.** Repair and onboarding both need it — adopting vump in a
repository whose files never agreed currently requires fixing them manually
first.

**Shape.** `vump set <version>`, writing every tracked file to exactly that
version. Same git integration as a bump. It should not need the files to agree
beforehand, since disagreement is precisely what it repairs.

**Open questions.**

- Should it refuse to move backwards without a flag? Probably not — an exact
  version was named, which is consent, matching `self update --to`.

### 2. Annotated and signed tags

**Problem.** Tags are created with `git tag <name>`, which makes a lightweight
tag: a bare pointer with no tagger, date, or message.

**Why it matters.** Many workflows expect annotated tags — `git describe`
prefers them, and some release tooling ignores lightweight tags entirely.
Signed tags are the natural companion for a repository that already signs
commits.

**Shape.** A `[git]` setting selecting the tag style, defaulting to the current
behavior so nothing changes silently. Annotated tags need a message template
alongside `tag_pattern`.

---

## Ideas, not yet decided

Worth recording. None have an agreed problem statement yet.

### Declarative version-file formats

Detection is by filename across three built-in formats. `pyproject.toml`,
`*.csproj`, `gradle.properties` and others need a per-entry extraction spec — a
path for structured formats, a pattern for the rest.

The reason this has not been designed: it changes the configuration schema, and
doing that well needs a real target format in hand rather than a guess at what
would be general enough.

### Update a lock file's own version entry

`Cargo.lock` records the version of the crate it locks. Bumping `Cargo.toml`
leaves the two disagreeing, which fails `cargo build --locked`. vump currently
reports this and stops there.

The tension: "vump never runs a package manager" is a firm non-goal, and rightly
so. But editing the lock's *own* entry is not resolving dependencies — it is the
same in-place version edit vump already performs on manifests. Whether that
distinction is real enough to act on is undecided.

### Signing the checksum file

Digests are published unsigned. That closes corruption and tampering in
transit, but not a compromise of the release pipeline itself, which could
publish matching binaries and digests together. Signing `SHA256SUMS` would
close that, and needs a key and somewhere to publish it.

### A distinct exit code for a refused install

`self update` refusing an unverifiable release exits 1, alongside every other
self-update failure. A caller wanting to distinguish "could not verify" from
"network failed" cannot. Whether that is worth another entry in the exit-code
table is undecided.

### Remembering an update channel

`--channel` is per-invocation. Someone tracking release candidates types it
every time. Persisting it needs installation-level state — a config directory
vump otherwise has no need for — which one setting does not obviously justify.

### Inputs on the check action

The composite action takes `version`, `config` and `vump-version`. Passing a
tag now selects its own project, so a `project` input is only needed for a
repository that verifies bare versions rather than tags.

### Per-project commit messages

`commit_message` accepts `{project}`, which distinguishes a monorepo's commits.
Whether a project also needs to *override* the whole message, as it can with
`tag_pattern`, has no motivating case yet: tags must be unique, commit messages
need not be.

---

## Decided against

### A `-y` / `--auto-approve` flag

Naming a subcommand already means vump never prompts, so a subcommand
invocation *is* the confirmation — the same reason `rm file` needs no
confirmation flag. A yes-flag would be surface area on top of a mechanism that
already does the job.

### Configuration in YAML or JSON

YAML's implicit typing is hazardous for a tool whose subject is exact version
strings: `1.0` becomes a float and loses its trailing zero, and bare `yes`/`no`
become booleans. JSON has no comments, which the generated configuration relies
on. Supporting several formats would also make the same tool look different in
every repository.

### A saved plan-then-apply workflow

Terraform's plan/apply exists because infrastructure changes are slow,
expensive, and reviewed by someone other than their author, often in a
different run. Bumping a version has none of those properties, and
`--dry-run --json` already emits the plan a caller would want to inspect.

Revisit only if a concrete workflow appears where a plan is reviewed
asynchronously by someone who did not produce it.
