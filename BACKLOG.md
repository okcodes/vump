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

### Refusing git work from a nested configuration

A `vump.toml` inside a repository whose own `vump.toml` sits above it commits
and tags into a repository it does not describe. `vump patch --through push`
run in `sandbox/npm/single-project` created a `v1.0.1` commit and tag in vump's
own repository, from a project versioned at 1.0.0 that has nothing to do with
vump's version.

Half of that has since been closed off: every sandbox configuration now carries
a `tag_pattern` that cannot match the `v*` the release workflow triggers on, so
an accidental tag can no longer impersonate a release. The structural half
remains — a nested configuration can still write to a repository that is not
its own.

**Refused, not warned.** A warning is visible in an interactive run and useless
everywhere else: a subcommand prints it into a log nobody is reading, and by the
time a `through = "push"` run emits one the tag is already on the remote. This
is the same reasoning that makes a release publishing no checksums a refusal —
a warning on an irreversible path is not a safeguard.

Nested configurations are legal, so the refusal needs an opt-out, and it is a
flag — `--allow-nested` — rather than a key in the nested file.

A configuration key would settle the question once and stay settled, which is
the failure mode: every later run from that directory is pre-approved, the
accidental one included. The flag asks again every time, so the person who has
forgotten which directory they are in is still stopped. Nobody re-reads a
`vump.toml` before typing `vump`, and a permanent disarm is how a safeguard
stops safeguarding.

This is not the kind of setting the flag rule in [`DESIGN.md`](DESIGN.md)
governs. That rule is about preferences with a default worth overriding for one
run; this is an acknowledgement of a hazard vump has already detected, which is
what `vump init --force` is too — and nothing would be gained by spelling that
one `allow_overwrite = true` in a file.

The friction a flag adds is the point rather than a cost to be minimized.
Nested configurations are not a layout to be accommodated: `[[project]]` exists
precisely so that a repository holding several things needs only one
configuration, and a repository that has grown a second one has given up
addressing projects by name, `vump status` over the whole repository, and any
way to trace a pushed tag back to its project. Having to type `--allow-nested`
on every such run is a standing reminder that a first-class feature is going
unused, and that the alternative to typing it is not typing it but restructuring
the configuration.

Which leaves the sandbox as the only nested configuration that should exist
anywhere, kept because those projects are meant to be run against by hand.

**The error message is where the work happens.** It must name the configuration
that was found, the outer one it sits under, and the repository the commit would
land in, so that it reads as "wrong directory" rather than as an obstacle to get
past. A refusal that only says what is forbidden invites reaching for the flag;
one that says where you are invites `cd`.

The refusal applies only when the run would reach a commit. `through = "none"`
touches nothing outside the working tree, and the sandbox has to stay pleasant
to use for what it is for.

Shape: after discovery, walk from the configuration's directory up to the git
root looking for another `vump.toml`. If one is found and the run would commit,
refuse with `Exit::Config`. Checked before anything is written, beside the
undeclared-lock check.

Undecided: `--allow-nested` has to be global rather than sit with the other git
flags, since those are on the bump subcommands only and a guided `vump` would
otherwise have no way to proceed at all.

## Ideas, not yet decided

Worth recording. None have an agreed problem statement yet.

### A shared version for a .NET solution

`Directory.Build.props` is where a solution with many projects usually keeps
one `<Version>`, inherited by every project beneath it. Reading it needs no new
code — it is the same `MSBuild` XML.

What is undecided is what happens when a project underneath also declares its
own `<Version>`, which overrides the inherited one. Tracking both would mean
vump writing two files that disagree by design. Needs a real solution in hand
rather than a guess at which layer wins.

### Deriving the version from git tags instead of files

Tools like MinVer compute a .NET package version from the nearest git tag, so
no file records it and nothing can fall out of sync.

Not adopted, and not a competitor so much as the opposite trade. It needs git
history at build time, which shallow CI checkouts and source tarballs do not
have; it leaves a checked-out tree with no readable version; and it does not
reach npm or Cargo, whose manifests must carry a version regardless — so a
polyglot repository would need both models at once. Most of all it removes the
second opinion: a mistyped tag becomes a correctly-built wrong version, with
nothing left to check it against, which is the failure `vump check` exists to
catch.

Worth revisiting only for a repository that is .NET alone and does not publish
source archives. `vump set "$(git describe --tags --abbrev=0)"` already covers
deriving a version from a tag for anyone who wants that direction.

### Python projects

`pyproject.toml` holds `[project].version`, which is the same in-place TOML
edit vump already performs. What is undecided is `uv`: it keeps a `uv.lock`
that records the project's own version, so the lock question arrives with it,
and whether `uv` rewrites more of that file than the version is unverified.

### npm workspaces

A Cargo workspace's shared lock is now written per member, matched by the
package name each manifest declares. `package-lock.json` has the same shape and
not the same solution: its `packages` map is keyed by *path*
(`"packages/api"`), not by package name, and a workspace member's version
appears both there and in the member's own `package.json`.

Undecided because no repository here needs it yet, and guessing at the mapping
between a declared manifest path and a `packages` key is exactly the kind of
inference that produced the original lock-file defect.

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

### Pushing what a bump created, separately

A bump that stops at `through = "tag"` leaves a commit and a tag to push by
hand. `git push --follow-tags` does it, but pushes *every* annotated tag
reachable — in a monorepo that can publish another project's tag that happened
to be sitting unpushed. vump's own push names the single ref it created, which
is strictly narrower.

A `vump push` would close that gap: read the current version, render the
project's tag pattern, push `HEAD` and that one tag. It needs no stored state,
since both are derivable from the files and the pattern.

Undecided because the repository that would feel it — a monorepo with several
projects tagged independently — does not exist here yet, and the single-project
case is served by `git push --follow-tags` today.

Not to be confused with a prompt between tagging and pushing, which was
considered and rejected: a blocking prompt owns the terminal, so it cannot
deliver the inspection it appears to offer. Stopping at `tag` and looking
around with a free shell is strictly better.

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

### Tracking yarn.lock or pnpm-lock.yaml

Neither records the project's own version, so a bump cannot make either stale.
Checked against both tools rather than assumed: with `"version": "1.2.3"` in
`package.json`, a generated `yarn.lock` and `pnpm-lock.yaml` each contain zero
occurrences of it. Yarn Berry pins its own workspace entry at a placeholder for
the same reason.

They record a dependency graph, and a version bump does not change one. An
earlier advisory named them anyway, which meant telling people to run an
install that would change nothing.

### A saved plan-then-apply workflow

Terraform's plan/apply exists because infrastructure changes are slow,
expensive, and reviewed by someone other than their author, often in a
different run. Bumping a version has none of those properties, and
`--dry-run --json` already emits the plan a caller would want to inspect.

Revisit only if a concrete workflow appears where a plan is reviewed
asynchronously by someone who did not produce it.
