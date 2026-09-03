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

### 1. `package-lock.json` moves with its manifest

**Problem.** `Cargo.lock` is now tracked and written in the same run as its
manifest. `package-lock.json` has the identical defect and is not: it records
the project's version at both `.version` and `.packages[""].version`, and
`npm ci` rejects a tree where those disagree with `package.json`.

**Why it matters.** It is the same failed release, in an ecosystem this project
ships from. A tag lands, `npm ci` fails in the release job, and recovery costs
a deleted tag.

**Shape.** A `Format::PackageLock` beside `Format::CargoLock`, written with the
existing depth-aware JSON scan. Two version sites rather than one, and
`lockfileVersion` 1, 2 and 3 place them differently — v1 has no `packages` map
at all. `companion_lock` in `app/mod.rs` gains the `package.json` case.

---

### 2. Workspace lock files

**Problem.** A lock covering several packages built from the repository records
no single project's version, so vump neither writes it nor asks for it to be
declared. A workspace member's bump therefore still leaves its lock stale —
the bug that was just fixed, in a layout vump cannot currently resolve.

**Why it matters.** `[[project]]` exists to serve monorepos, and a Cargo
workspace is the most common Rust shape of one.

**Shape.** The ambiguity is only in the lock file: the manifest names its
package, so the right `[[package]]` entry is findable once the two are read
together. That means resolving a lock against the manifest declared beside it
rather than in isolation, which is a change to how `read_project_versions`
composes. Sibling crates that pin each other by version need their
requirements bumped too, and that part may be worth refusing rather than
building.

---

### 3. Annotated and signed tags

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

Detection is by filename across the built-in formats. `pyproject.toml`,
`*.csproj`, `gradle.properties` and others need a per-entry extraction spec — a
path for structured formats, a pattern for the rest.

The reason this has not been designed: it changes the configuration schema, and
doing that well needs a real target format in hand rather than a guess at what
would be general enough.

### Enforcing provenance verification, not just publishing it

Releases carry a signed provenance attestation, and anyone can check it with
`gh attestation verify`. Nothing in the tooling *requires* that check: the CI
action and self-update both enforce checksums instead.

The reason is portability, not doubt about the value. Checksums need only
`curl` and a hash utility, both present anywhere the action runs, self-hosted
runners included. Verifying an attestation needs a recent `gh`, and verifying
one from `vump self update` would need a Sigstore implementation in Rust — the
crates are immature — or shelling out to `gh`, which a user's machine may not
have.

Worth revisiting if the Rust Sigstore ecosystem matures, or if the action's
consumers are known to have `gh` available.

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

### Running the package manager to refresh a lock file

vump writes a lock file's own version entry, which needs no network and no
knowledge of the dependency graph. Running `npm install` or `cargo check` to
do it instead would mean unbounded runtime, network access, and — for npm —
executing arbitrary lifecycle scripts, all inside a tool whose job is editing a
version string. It would also make "your build is broken" one of vump's failure
modes, and require guessing which package manager a repository uses.

The line: vump may write a value it already computed; it may never resolve
dependencies.

### A command to resume an interrupted bump

Proposed when a bump could commit and tag before reporting that a lock file had
gone stale, leaving a half-finished release to clean up by hand.

The failure it would recover from no longer happens: everything knowable before
writing is now checked before writing, so a run that cannot finish cleanly does
nothing at all. A resume command would also need persisted state — which the
design rejects for `--channel` on the same grounds — and would have to decide
which files and which lines to commit, questions with no defensible answer.
Running the fix leaves the tree dirty, which is itself refused.

Where a repair is genuinely needed, `vump set <version>` writes every tracked
file and requires no prior agreement between them. That is the resume command.

### A saved plan-then-apply workflow

Terraform's plan/apply exists because infrastructure changes are slow,
expensive, and reviewed by someone other than their author, often in a
different run. Bumping a version has none of those properties, and
`--dry-run --json` already emits the plan a caller would want to inspect.

Revisit only if a concrete workflow appears where a plan is reviewed
asynchronously by someone who did not produce it.
