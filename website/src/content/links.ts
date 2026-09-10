const REPO = 'https://github.com/okcodes/vump';

export const links = {
  repo: REPO,
  readme: `${REPO}#readme`,
  releases: `${REPO}/releases`,
  design: `${REPO}/blob/main/DESIGN.md`,
  contributing: `${REPO}/blob/main/CONTRIBUTING.md`,
  backlog: `${REPO}/blob/main/BACKLOG.md`,
  action: `${REPO}/tree/main/.github/actions/check`,
  sandbox: `${REPO}/tree/main/sandbox`,
} as const;
