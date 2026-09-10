# vump website

The landing page for [vump](https://github.com/okcodes/vump). One page, built
to static files and destined for GitHub Pages.

```bash
npm install
npm run dev          # http://localhost:5173
npm run check        # typecheck, lint, format, build — what CI will run
```

| Script                    | Does                                                          |
| ------------------------- | ------------------------------------------------------------- |
| `dev`                     | Vite dev server with hot reload                               |
| `build`                   | Type-checks, then writes `dist/`                              |
| `preview`                 | Serves `dist/` as a deploy would                              |
| `typecheck`               | `tsc -b`, no emit                                             |
| `lint`                    | oxlint; any warning fails                                     |
| `format` / `format:check` | Prettier over everything                                      |
| `check`                   | All four, in order                                            |
| `og`                      | Regenerates `public/og.png` and `public/apple-touch-icon.png` |

## What is where

| Path                       | Holds                                                                                                                   |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `src/content/`             | Every fact the page states: terminal runs, tracked files, version rules, exit codes, links. Nothing here renders.       |
| `src/components/sections/` | One file per numbered section, in page order.                                                                           |
| `src/components/ui/`       | The primitives: terminal pane, code block, section frame, reveal, buttons.                                              |
| `src/lib/`                 | Hooks and the two small engines — a segment model for command output, and a highlighter for the four snippet languages. |
| `src/index.css`            | The whole design system: palette, type, motion.                                                                         |
| `scripts/og.mjs`           | Renders the social card from the same values, offline.                                                                  |

**Content is separate from layout on purpose.** Every terminal pane on the page
reproduces what the binary actually prints — the marks, the column alignment,
the wording of the errors. When vump's output changes, `src/content/runs.ts` is
the only file to revisit.

**The page states no version number.** The release chip fetches the newest
release from the GitHub API as the page loads, and renders nothing when that
fails. A version written into a marketing site is a version nobody bumps, which
is the defect vump exists to prevent.

## Design system

Colours are semantic roles — `ground`, `ink`, `muted`, `line`, `signal` — defined
once in `src/index.css` and flipped by a `data-theme` attribute on the document
element. No component names a literal colour, so the two themes cannot drift
apart. A script in `index.html` resolves the theme before first paint; an
explicit choice outranks the system preference.

Terminal surfaces are held dark in both themes. Rendered command output should
read as output, not as a styled quotation.

The accent is spent on one thing: what changed. A new version number, a passing
mark, a highlighted line in a snippet. Amber marks a refusal, red a failure, and
nothing else is coloured at all.

## Lint configuration

Four rules are off in `.oxlintrc.json`, each because it does not fit this
codebase rather than because it was inconvenient:

| Rule                       | Why                                                                                                                        |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `react/no-array-index-key` | Rendered lines and tokens are static arrays that never reorder; the index is the correct identity.                         |
| `max-lines-per-function`   | Counts JSX as logic. A section component is markup, not complexity.                                                        |
| `import/max-dependencies`  | `App.tsx` composes every section; that is its whole job.                                                                   |
| `require-unicode-regexp`   | The highlighter's patterns run over ASCII snippets written in this repository, and `u` mode outlaws the escapes they need. |

Everything else — correctness, suspicious, perf and pedantic — is on, and a
warning fails the run.

## Deploying to GitHub Pages

Not wired up yet, deliberately: the domain is not settled, and the base path is
baked into every asset URL at build time.

Two values decide it, both in `.env`:

| Variable         | For a custom domain   | For `okcodes.github.io/vump`     |
| ---------------- | --------------------- | -------------------------------- |
| `VITE_BASE_PATH` | `/`                   | `/vump/`                         |
| `VITE_SITE_URL`  | `https://your.domain` | `https://okcodes.github.io/vump` |

A custom domain also needs `public/CNAME` holding the bare hostname, which ships
to `dist/` untouched. `public/.nojekyll` is already there, so Pages serves the
build as-is instead of running it through Jekyll.

When the domain is decided, add `.github/workflows/website.yml`:

```yaml
name: website

on:
  push:
    branches: [main]
    paths: ['website/**', '.github/workflows/website.yml']
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: pages
  cancel-in-progress: true

jobs:
  build:
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: website
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 24
          cache: npm
          cache-dependency-path: website/package-lock.json
      - run: npm ci
      - run: npm run check
      - uses: actions/upload-pages-artifact@v3
        with:
          path: website/dist

  deploy:
    needs: build
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deploy.outputs.page_url }}
    steps:
      - id: deploy
        uses: actions/deploy-pages@v4
```

Then set Pages → Source → GitHub Actions in the repository settings.

The site is not versioned and vump does not track its `package.json`, which
declares no version at all. It ships when `main` moves, not when a tag does.
