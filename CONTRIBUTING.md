# How work is done here

Process rather than code style: how a change gets from a branch to a release.
For how the code itself is written and judged, see
[`ENGINEERING.md`](ENGINEERING.md).

## The local loop

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs exactly this, on Linux, macOS and Windows. Run it locally before
pushing — hosted runners are slow, and discovering a formatting error on one
costs minutes for something that costs seconds here. Use CI to confirm genuine
platform differences, not ordinary mistakes.

## Branches and pull requests

- Branch from `main`, named for the change (`set-version`,
  `release-checksums`).
- **Open a pull request when a coherent set of work is done, then pause for
  review.** A pull request is a checkpoint, not a formality: it is where review
  catches what tests cannot.
- The pull request description explains what changed and why it is shaped that
  way. If the shape changes during review, update the description — a
  description that no longer matches its own branch misleads whoever reads it
  next.

## Commits

- **Small and frequent.** Every commit compiles and passes tests on its own.
- Merges preserve full branch history rather than squashing, so every commit
  message is permanent. Write them for someone reading in a year.
- Conventional prefixes: `feat`, `fix`, `refactor`, `chore`, `ci`, `docs`,
  `test`.
- The subject is imperative and specific. The body says *why*, not what — the
  diff already says what.

## Releases

vump versions itself with vump.

```bash
vump patch --tag --push        # or: alpha, beta, rc, release
```

Pushing the tag runs the release workflow, in this order:

1. **Verify** — the tag is checked against the version in source before
   anything is built, then format, lint and tests run. A tag that lies costs
   nothing.
2. **Build** — six targets, including static musl Linux binaries and a Windows
   arm64 cross-compile.
3. **Sign** — the macOS universal binary is signed and notarized.
4. **Checksum** — `SHA256SUMS` is produced *after* signing, because signing
   rewrites the binary and a digest taken earlier would describe an artifact
   nobody receives.
5. **Attest** — keyless SLSA provenance, signed with a short-lived certificate
   from the workflow's own identity and recorded in a public transparency log.
   There is no key to store, rotate or leak.
6. **Publish** — the release and its assets.

**Cut an alpha whenever a change is worth trying on a real machine.** Something
that passes tests and looks finished is not the same as something used once in
anger, and an alpha costs one tag.

Asset names (`vump-<os>-<arch>`, plus `vump-darwin-universal`) are a contract:
the CI check action and in-place self-update both resolve them.

## Where things get written down

Keeping the documents true is part of the change, not follow-up work.

| Document | Update it when |
| --- | --- |
| [`DESIGN.md`](DESIGN.md) | Behavior or architecture changes — in the same pull request |
| [`README.md`](README.md) | The user-facing surface changes |
| [`BACKLOG.md`](BACKLOG.md) | Something is proposed, deferred, or ruled out |
| [`ENGINEERING.md`](ENGINEERING.md) | A standard changes, together with the example that motivated it |
| [`CLAUDE.md`](CLAUDE.md) | The document map or the conventions in it change |

Two rules keep `DESIGN.md` and `BACKLOG.md` from drifting into each other:

- `DESIGN.md` describes what is true **now**. It is authoritative: where it and
  the code disagree, one of them is defective and the disagreement gets
  resolved deliberately rather than by assuming the code is right.
- `BACKLOG.md` holds only what is **not** settled. When an item is built, its
  rules move into `DESIGN.md` and the backlog entry is deleted — an entry
  describing shipped behavior is a second, competing specification.

**Decided against** entries exist so a question is not reopened without new
information. Reopening one is fine; say what changed.
