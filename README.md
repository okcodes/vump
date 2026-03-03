# vump

**vump** is a standalone CLI tool for bumping semver versions across multiple files in a monorepo — atomically, interactively, and safely.

## Install

```sh
go install github.com/okcodes/vump@latest
```

Or download a pre-built binary from [GitHub Releases](https://github.com/okcodes/vump/releases).

## Quick Start

1. Create a `vump.toml` at your repo root:

```toml
[[files]]
path = "VERSION"

[[files]]
path = "ui/package.json"

[[files]]
path = "api/Cargo.toml"
```

2. Run interactively:

```sh
vump
```

vump reads the current version from each file, checks they're in sync, and guides you through the bump.

## Commands

```
vump                       Fully interactive
vump patch                 Bump patch non-interactively (1.2.3 → 1.2.4)
vump minor                 Bump minor             (1.2.3 → 1.3.0)
vump major                 Bump major             (1.2.3 → 2.0.0)
vump alpha                 Start or increment alpha pre-release
vump beta                  Start or increment beta pre-release
vump rc                    Start or increment RC pre-release
vump release               Drop pre-release suffix (1.2.3-rc.1 → 1.2.3)
```

## Flags

| Flag | Description |
|---|---|
| `--dry-run` | Preview what would change, write nothing |
| `--force` | Bypass backwards pre-release guard |
| `--from <patch\|minor\|major>` | Required with alpha/beta/rc from a stable version (non-interactive) |
| `--commit` | `git add` + `git commit` after bumping |
| `--tag` | `git add` + `git commit` + `git tag` (implies `--commit`) |

### Examples

```sh
# Non-interactive patch bump + commit
vump patch --commit

# Start an alpha pre-release off a minor bump, then tag
vump alpha --from minor --tag

# Preview a major bump without touching files
vump major --dry-run

# Force a backwards pre-release (rc → beta)
vump beta --force
```

## Supported File Types

Detection is by filename only.

| Filename | Format | Version Field |
|---|---|---|
| `package.json` | JSON | `.version` |
| `Cargo.toml` | TOML | `[package].version` |
| `VERSION` | Plain text | Entire file content |

Any other filename causes a startup error.

## Semver Bump Rules

Pre-release order: `alpha` < `beta` < `rc`

| Current | Command | Result |
|---|---|---|
| `1.2.3` | `patch` | `1.2.4` |
| `1.2.3` | `minor` | `1.3.0` |
| `1.2.3` | `major` | `2.0.0` |
| `1.2.3` | `alpha --from patch` | `1.2.4-alpha.0` |
| `1.2.3-alpha.0` | `alpha` | `1.2.3-alpha.1` |
| `1.2.3-alpha.2` | `beta` | `1.2.3-beta.0` |
| `1.2.3-beta.1` | `rc` | `1.2.3-rc.0` |
| `1.2.3-rc.1` | `release` | `1.2.3` |
| `1.2.3-beta.0` | `alpha` | ❌ error (backwards — use `--force`) |

## Git Integration

Configure in `vump.toml`:

```toml
[git]
commit = true
commit_message = "chore: bump version to v{new_version}"
tag = false
tag_pattern = "v{new_version}"
```

Or via flags (`--commit`, `--tag`). CLI flags override config file values.

`--tag` implies `--commit`. vump will never `git push` — it prints the push command for you instead.

**Dirty tree check:** if `--commit` or `--tag` is active, vump checks for uncommitted changes first and fails fast. Only the files declared in `vump.toml` are staged.

## Out-of-Sync Files

If files contain different versions, vump presents them interactively and asks which to use as the base. All files are then written with the new version.

## Lock File Warnings

After bumping:
- `package.json` updated → warns if `package-lock.json` / `yarn.lock` / `pnpm-lock.yaml` exists
- `Cargo.toml` updated → warns if `Cargo.lock` exists

vump never runs `npm install` or `cargo build`.

## Development

```sh
go build ./...          # build
go test ./...           # unit + E2E tests
go vet ./...            # linter
```
