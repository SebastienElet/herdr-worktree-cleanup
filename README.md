# Herdr Worktree Cleanup

Version 1.0.0 is prepared locally; registry publication and native release qualification are pending.

Remove an eligible linked Git worktree when its Herdr workspace closes. The plugin retains ID
`dotfiles.worktree-cleanup`, event `workspace.closed` and Docker cleanup `off` by default.
Git refuses dirty, untracked, locked and foreign checkouts; detached HEADs are preserved. Ignored
files can disappear with an otherwise clean checkout: lock any worktree used as a deployment source.
Deleting a checkout preserves its branch; a rollback cannot recreate deleted files.

Target: macOS ARM64, Bun 1.4.3, Git and Herdr (declared minimum 0.9.3, not a qualification of all
later versions). Docker is optional. The explicit `--docker-cleanup=stop` mode uses only container
IDs returned for an exact Compose `working_dir` label match; Docker failures do not block closure.
The issue launcher and its skill are independent and are not installed by this package.

After publication and qualification, install in a durable directory:

```sh
mkdir -p "$HOME/.local/share/herdr-worktree-cleanup"
cd "$HOME/.local/share/herdr-worktree-cleanup"
bun install --exact --ignore-scripts @sebastienelet/herdr-worktree-cleanup@1.0.0
herdr plugin link "$PWD/node_modules/@sebastienelet/herdr-worktree-cleanup"
```

Keep the installation's manifest and Bun lockfile; reproduce it with
`bun install --frozen-lockfile --ignore-scripts`. Never link a persistent plugin into a `bunx` cache.
Installation runs no lifecycle scripts, cleanup or native handler.

For substitution, suspend workspace closures, record `herdr plugin list`, and inspect ownership.
An existing different registration is a collision, not permission to overwrite it. Explicitly disable
the old handler with `herdr plugin disable dotfiles.worktree-cleanup`, then remove only its owned
registration with `herdr plugin unlink dotfiles.worktree-cleanup` before linking the new package.
Keep exactly one handler active. Verify closure of a disposable clean worktree and preservation of
dirty, locked, detached and foreign checkouts before removing old sources. Preserve unrelated
configuration, data and worktrees. For rollback, disable/unlink the new registration and restore the
recorded prior link or exact package. Uninstall with explicit disable/unlink, then
`bun remove @sebastienelet/herdr-worktree-cleanup` in its installation directory; never close a
workspace or delete data as an installation repair.

For source contribution:

```sh
bun install --frozen-lockfile --ignore-scripts
bun test
bun run format:check
bun run lint
bun run typecheck
bun pm pack --ignore-scripts
```

CI transfers the existing TypeScript checks and tests to macOS/Ubuntu. Tests exercise real disposable
Git repositories and substitute Docker; they do not prove native Herdr event delivery or real Docker.
No assembled native release has been qualified by this local preparation.

The manual publication job in `ci.yml` on `main` uses only `npm publish --provenance` in CI with
OIDC. Configure the npm trusted publisher against the future public repository
`SebastienElet/herdr-worktree-cleanup`, workflow `ci.yml`; no token fallback is configured.
See [npm trusted publishing](https://docs.npmjs.com/trusted-publishers/) and
[Bun archive preparation](https://bun.sh/docs/pm/cli/pm#pack). After an unknown publication result,
inspect the registry version, integrity and provenance before any retry; do not unpublish.
