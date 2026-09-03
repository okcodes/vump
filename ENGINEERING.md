# Engineering standards

How code is written and judged in this repository. Every rule here earned its
place by being violated first: the examples are real, and naming them is the
point — a standard without a case behind it is a preference.

[`DESIGN.md`](DESIGN.md) says what vump does. This says how it should be built.

## 1. A type says what a value means

**If a value's meaning depends on which caller produced it, the type is
wrong.**

`BumpPlan` once carried the version being moved away from as
`Option<Version>`, where `None` meant "this plan came from `set`, whose files
may disagree, so there is no single one". Three things gave it away:

- It encoded a distinction between *operations* in a *nullable field*.
- Its documentation had to name the caller to explain the absence. A type
  needing prose about who constructed it is not describing itself.
- The absence leaked outward: every renderer had to handle a case that only
  one of two callers could ever produce.

The fix was not a parallel `SetPlan` — that would have duplicated the half the
two operations genuinely share. It was to extract that half:

```rust
pub struct ChangeSet { project, target, files, git }       // what both produce

pub struct BumpPlan { from: Version, changes: ChangeSet }   // plain, not Option
pub fn set(..) -> ChangeSet                                 // no `from` at all
```

`set` does not get an awkward `None`. It has no such field, which is the
honest model: a set operation has no origin version, and now cannot be asked
for one.

The general moves:

- **Extract what variants share; do not duplicate what they do not.** Reach for
  a common struct before a parallel one.
- **`Option` is for genuine absence** — a configuration key nobody set, a tag
  that will not be created. It is not a way to spell "sometimes this means
  something else".
- **Closed sets are enums**, not booleans, strings, or integers. `PreLabel`,
  `Transition` and `Channel` exist so an invalid combination cannot be
  constructed.
- **Configuration and arguments are structs**, parsed once at the edge. No
  magic strings threaded through call sites.

## 2. Derive what can be derived

**Two stored fields that must agree will eventually disagree.** Make one a
function of the other, and the disagreement becomes unrepresentable.

- `ChangeSet::common_origin()` is computed from the per-file versions rather
  than stored beside them. The per-file versions are the truth; a cached
  summary could contradict them.
- `FileChange` records only the version a file holds today. The version it
  will hold is the change set's target, identical for every file by
  construction — storing it per file would invite exactly the drift that
  keeping versions in step is meant to prevent.
- Release maturity is read from the version string, not from how a release was
  flagged when published, so the two can never disagree.

## 3. Refuse rather than guess

**When vump cannot know, it says so and names the next action.** It does not
pick the likely option, and it does not fall back to asking — a subcommand
never prompts.

| Situation | Response |
| --- | --- |
| Tracked files disagree | Error listing the disagreement. `set` is the repair. |
| `patch` on a pre-release | Error naming the two explicit steps instead |
| A pre-release from stable without `--from` | Error; the flag is required |
| Two projects claiming one tag | Reported, never attributed to a guess |
| A release publishing no checksums | **Refused, not warned** |

The last one is the sharpest form of the rule. Self-update downloads a binary
and then executes it as the user, and in CI does so inside a job that can hold
signing secrets. A warning on that path is not a safeguard, and the cost —
releases predating checksums cannot be installed — is worth paying.

Where a rule refuses something, the error message names the way forward. An
error a user cannot act on is a defect.

## 4. Comments and documentation

**Comments answer "why this, and not the obvious alternative".** What the code
does is the code's job.

Never write how the code came to be. No development history, no "previously",
no "changed to", no reference to a discussion, no TODO left as a bookmark. A
reader six months out has no access to any of that context and does not need
it: the current reasoning is what keeps the code correct.

```rust
// A regex over `^version\s*=` also matches a dependency's own pin, which is
// how the previous implementation corrupted `[dependencies.serde]`. Editing
// the document structurally cannot reach outside `[package]`.
```

That comment earns its place: it names a plausible simpler approach and the
concrete failure that rules it out. Compare with what does not:

```rust
// Changed this to use toml_edit instead of a regex as discussed
// Loop over the files
// TODO: revisit this later
```

Doc comments on public items are enforced by `missing_docs`. They state the
contract and the failure modes, not the implementation steps. Module docs say
what the module is for and, where it matters, what it deliberately does not
do — `app::set` opens by explaining why it has no notion of a version being
moved *from*, which is the question every reader arrives with.

Markdown files follow the same rules and one more: **they describe the tool,
not the people building it.** No personal context, no workplace details, no
residue of how a decision was reached beyond the reasoning itself. Voice is
plain and declarative — state the rule, then the reason in a clause. These are
read by humans as often as by machines.

## 5. Tests

Three layers, each with a distinct job, described in [`DESIGN.md`
§7](DESIGN.md#7-testing): pure domain units, use cases against in-memory
fakes, and end-to-end runs of the compiled binary.

- **A fixed bug arrives with the test that would have caught it.** Both file
  corruption bugs carried over from the previous implementation —
  `package_json_ignores_nested_version_keys` and
  `cargo_ignores_dependency_versions` — are named for the mistake, not for the
  function under test.
- **Test against external truth, not against yourself.** The hasher is checked
  against the published SHA-256 vectors. Asserting that our hash equals our
  hash would pass for any implementation, correct or not.
- **Verification that succeeds silently needs a negative control.** Signature
  and checksum checks are meaningless until you have watched one fail on
  purpose: flip a byte, assert the failure.
- **Name the behavior, not the function.** `moves_backwards_without_complaint`
  and `a_missing_file_is_reported_with_its_declared_path` say what is
  guaranteed. A test comment is welcome when it explains why a case matters;
  one restating the assertion is noise.

## 6. Lints

```toml
[lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"

[lints.clippy]
pedantic = { level = "warn", priority = -1 }
```

CI runs `cargo clippy --all-targets -- -D warnings`, so a warning is a failure.

**An `allow` carries a comment saying why**, and is scoped as narrowly as it
can be. There is currently one, on the struct holding the git flags: the usual
objection to several booleans in one struct is unreadable call sites, which
does not apply when every field is a distinct flag named at the point of use.
Silencing a lint without stating the case is how a codebase stops meaning what
its configuration claims.

## 7. Dependencies

A small surface, and a reason for each addition.

- **Prefer `rustls` to anything OpenSSL-backed.** Release binaries link
  statically against musl, which native TLS does not cooperate with.
- **git is a subprocess, not a library.** This inherits the user's real
  configuration, credential helpers, hooks and SSH setup for free.
- **Delegate platform differences rather than reimplementing them.** Replacing
  a running executable differs between Unix and Windows; that belongs in a
  library that tracks it.
- **Formatting-preserving edits use a real document model** (`toml_edit`, a
  depth-aware JSON scan) rather than pattern replacement. Correctness here
  outranks the elegance of the parsing approach: a tool that reformats a
  `package.json` as a side effect of bumping it will not be tolerated in a
  repository.
