# Backlog

Work that is not built yet, and decisions taken against building things.

[`DESIGN.md`](DESIGN.md) describes how vump behaves *now* and is authoritative
for the code. This file is the opposite: it holds what is not settled. When an
item here is built, its rules move into `DESIGN.md` and the entry is deleted.

**Decided against** entries exist so a question is not reopened without new
information. If you find yourself proposing one of them, say what changed.

**A fix small enough to make now does not come here.** This file holds work
that needs a decision, a design or a refactor. It is not a holding pen for
things that could have been done in the pull request that found them — an entry
reading "rename this" or "this comment is stale" costs more to file and re-read
than to fix.

An entry is ready to build when its *Problem* is agreed. The *Shape* is a
starting point, not a specification — expect it to change while building.

---

## Ready to build

Empty. Work arrives as an idea below and moves here once its problem is
agreed, at which point it is ranked by value.

## Ideas, not yet decided

Worth recording. None have an agreed problem statement yet.

### A shared version for a .NET solution

`Directory.Build.props` is where a solution with many projects usually keeps
one `<Version>`, inherited by every project beneath it. Reading it needs no new
code — it is the same `MSBuild` XML.

What is undecided is what happens when a project underneath also declares its
own `<Version>`, which overrides the inherited one. Tracking both would mean
vump writing two files that disagree by design.

The reason this waited — "needs a real solution in hand rather than a guess at
which layer wins" — no longer holds. `sandbox/cs/multi-project` is a real
solution, so which layer wins is a question `dotnet build` can be made to
answer rather than one to reason about: add a `Directory.Build.props` carrying
a `<Version>`, let one project beneath it declare its own, and read what each
assembly ends up with.

What that leaves is a design question rather than an unknown. Probably: track
the props file *or* the projects, never both, and refuse a configuration
declaring a project whose version an ancestor overrides — the same shape as
refusing a manifest and lock that disagree.

The versioning model itself was checked against .NET 10 and needs nothing new:
`<Version>`, `<VersionPrefix>` and `<VersionSuffix>` are unchanged, and
`Directory.Packages.props` centralizes *dependency* versions, which is a
different file and not this.

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
edit vump already performs. What is undecided is `uv`, which is the tool that
would be used here: it keeps a `uv.lock` recording the project's own version,
so the lock question arrives with it.

Settling it is an experiment, not a discussion, and the same one that settled
npm and Cargo: put a project in `sandbox/py/`, bump the version by hand, run
`uv lock`, and diff. If the only change is the project's own version, the lock
is trackable on exactly the terms `Cargo.lock` and `package-lock.json` are —
computable with no network and no knowledge of the dependency graph. If `uv`
rewrites more than that, it is not, and `pyproject.toml` is tracked alone.

Low priority by the only measure that matters here: barely any Python is
written in this repository's orbit, so it waits behind formats that are.

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

### A Rust project in the sandbox

The sandbox covers npm and C#. Rust is missing, and the argument at the time
was that this repository is itself the Cargo example — it tracks `Cargo.toml`
and `Cargo.lock` and is exercised on every release.

That argument is weaker than it looked. The repository demonstrates a
single-crate project only, so the shape actually worth showing by hand — a
workspace, where the lock holds one entry per member and a project writes only
the entries its own manifests name — has no worked example anywhere outside the
test fixtures.

The reason not to rush: a crate inside this repository's tree is not inert the
way an npm or C# project is. It would need excluding from the workspace, and a
mistake there breaks `cargo build` for the tool itself.

### Inputs on the check action

The composite action takes `version`, `config` and `vump-version`. Passing a
tag now selects its own project, so a `project` input is only needed for a
repository that verifies bare versions rather than tags.

There is no `allow-nested` input either, so a repository whose `config` input
points at a nested `vump.toml` now fails in CI. That is arguably the right
outcome — it surfaces the layout rather than verifying quietly against the
wrong project — but it is a consequence that was not chosen deliberately, and
the alternative is one input.

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

### Reporting which `vump.toml` answered

Built and shipped in 0.4.0: `status` printed the configuration in effect above
the versions, named relative to the configuration above it. Removed in the next
release.

The case for it was that discovery searches upward, so a report that omits the
file looks the same from everywhere. But one configuration is the only
supported layout, and there it prints `vump.toml` — a line carrying nothing.
The line only says something when configurations are stacked, which is the
arrangement `[[project]]` exists to prevent, so the feature spent its whole
budget on a layout the tool refuses.

Naming it also needed a frame, and every frame was wrong somewhere. Relative to
the working directory, a nested file and a repository's own both read
`vump.toml`. Relative to the repository root, git decides a question that has
nothing to do with git. Relative to the configuration above — what shipped —
reads correctly with two configurations and misleads with three: from
`mid/deep/inner/` the report said `deep/inner/vump.toml`, a fragment anchored to
a file the reader cannot see. Anchoring at the outermost configuration instead
would mean searching to the filesystem root to render one line.

Only `status` printed it, so the same question went unanswered by `check`,
whose verdict is the one that gets believed. Extending it to every command
would have multiplied a cost already not worth paying once.

What closes the case: the nesting refusal already names both files, in full,
and it fires exactly when the answer matters. Worth reopening only if a CI log
has to be read back to work out which project was verified — and the fix then
is `check` naming its project, not any command naming its configuration.

### Announcing that a flag overrode configuration

Proposed when flags became two-directional: if `vump.toml` says `through =
"push"` and `--through tag` is passed, should the run say so?

No. Output reports the effective plan, never where each decision came from. The
flag is in the command the caller just typed, the result already names the
commit, the tag and the push outcome, and `--dry-run` exists for asking in
advance. "Configuration was overridden" is a notice with no action attached to
it, which is the same test that governs error messages.

The guided run needs it least of all: it takes no git flags, so nothing there
can be overridden.

### A saved plan-then-apply workflow

Terraform's plan/apply exists because infrastructure changes are slow,
expensive, and reviewed by someone other than their author, often in a
different run. Bumping a version has none of those properties, and
`--dry-run --json` already emits the plan a caller would want to inspect.

Revisit only if a concrete workflow appears where a plan is reviewed
asynchronously by someone who did not produce it.
