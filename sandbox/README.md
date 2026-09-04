# Sandbox

Working projects to run vump against by hand, one per ecosystem and per shape.
They are real: the npm lock files came from `npm install`, the C# projects from
`dotnet new`, and each prints its own version when run. Reading them shows what
a `vump.toml` looks like for that ecosystem; running vump in them shows what a
bump actually rewrites.

This is not test coverage. The suite in `tests/` and the fixtures in
`src/domain/testdata/` prove correctness. These exist to be looked at.

| Path | Shows |
| --- | --- |
| [`npm/single-project`](npm/single-project) | One package, manifest and lock moving together |
| [`npm/multi-project`](npm/multi-project) | Two packages released independently, with per-project tags |
| [`cs/single-project`](cs/single-project) | `<Version>` in a `.csproj`, and what stays put around it |
| [`cs/multi-project`](cs/multi-project) | Two C# projects versioned independently |

## Git is off, deliberately

These projects have no repository of their own — they sit inside vump's. A
commit or tag made here lands in **this** repository, so every `vump.toml` in
the sandbox sets `commit`, `tag` and `push` to `false`.

That leaves nothing untested. Commits, tags and pushes are covered by the
end-to-end suite, which creates a throwaway repository per test and asserts on
what git actually built. The sandbox covers the half those tests do not show a
human: the file rewrites.

If you do want to watch the git side by hand, clone into a scratch directory
first and let the tags die with it:

```bash
git clone . /tmp/vump-scratch && cd /tmp/vump-scratch/sandbox/npm/single-project
```

Passing `--commit` or `--tag` here overrides the configuration for that run, as
flags are meant to. Nothing stops you; it will just be this repository's
history that grows a tag.

## Trying it

```bash
cd sandbox/npm/single-project
vump status                 # what is recorded now
vump minor --dry-run        # what a bump would rewrite
vump minor                  # rewrite it
node index.js               # the new version, from the manifest
```

The multi-project directories address their projects by name, and a tag
identifies its own project:

```bash
cd sandbox/npm/multi-project
vump status                          # every project at a glance
vump patch --project project-a
vump check project-a-v1.0.1          # the tag says which project to verify
```

For C#, the version reaches the assembly:

```bash
cd sandbox/cs/single-project
vump alpha --from minor
dotnet run --project Demo            # Demo 1.1.0-alpha.0
```

## Putting it back

Experiments are meant to be thrown away:

```bash
git checkout -- sandbox
```
