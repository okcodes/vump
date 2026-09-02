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

Ranked. The first one closes a hole in something already shipped; the rest add
capability.

### 1. Per-project tag patterns

**Problem.** `tag_pattern` lives under `[git]` and applies to the whole
repository. A monorepo with independently-versioned projects therefore tags
every one of them `v1.2.3`. Two projects reaching the same version collide
outright, and `vump check "$GITHUB_REF_NAME"` cannot tell which project a
pushed tag refers to.

**Why it matters.** Multi-project support shipped without its release story.
Tag verification is the thing vump exists for, and it does not currently work
for the repositories the `[[project]]` feature was added to serve.

**Shape.**

```toml
[[project]]
name = "api"
files = ["services/api/Cargo.toml"]
tag_pattern = "api-v{new_version}"   # falls back to [git].tag_pattern
```

`check` should infer the project by matching a tag against each project's
pattern — treat the pattern as a template with `{new_version}` as the capture,
and read the version out of the tag. Explicit `--project` stays available to
disambiguate.

**Open questions.**

- What happens when two patterns match one tag? Erroring is probably right,
  since the answer is genuinely ambiguous.
- Is a `{project}` placeholder worth adding, so one repository-wide pattern
  (`{project}-v{new_version}`) covers the common case without repetition?
- Should `check` with no `--project` in a multi-project repository verify *all*
  projects, or refuse? Today it refuses.

### 2. Verify downloaded release artifacts

**Problem.** `download-vump.sh` fetches a binary over HTTPS and executes it in
CI, where the Apple signing certificate and keychain password are in scope.
There is no integrity check. `vump self update` has the same shape: download,
mark executable, replace the running binary. macOS artifacts are notarized;
Linux and Windows have nothing.

**Why it matters.** This is the highest-privilege code path in the project. A
compromised or truncated download is executed with secrets available.

**Shape.** Publish a `SHA256SUMS` asset from the release workflow, alongside
the binaries. Verify it in `download-vump.sh` and in the self-update adapter
before replacing anything. Failing the check must abort, not warn.

**Open questions.**

- Signing the checksum file itself is the stronger version, but needs a key
  and somewhere to publish it. Plain checksums close the truncation and
  tampering-in-transit cases and are worth having regardless.
- Should `self update` refuse outright when no checksum is published for a
  release, or fall back with a warning? Refusing is safer; it breaks updating
  *to* older releases that predate the checksums.

### 3. Set an exact version

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

### 4. Annotated and signed tags

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

### Remembering an update channel

`--channel` is per-invocation. Someone tracking release candidates types it
every time. Persisting it needs installation-level state — a config directory
vump otherwise has no need for — which one setting does not obviously justify.

### Inputs on the check action

The composite action takes `version`, `config` and `vump-version`. A
multi-project repository will likely want a `project` input, and this depends
on item 1 above.

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
