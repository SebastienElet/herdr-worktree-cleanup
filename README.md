# Herdr Worktree Cleanup

Herdr plugin that removes an eligible linked Git worktree when its workspace closes.

Requires macOS ARM64, Bun 1.4.3 and Herdr 0.9.3 (declared minimum; later versions are unqualified).

## Install

```sh
mkdir -p "$HOME/.local/share/herdr-worktree-cleanup"
cd "$HOME/.local/share/herdr-worktree-cleanup"
bun install --exact --ignore-scripts @sebastienelet/herdr-worktree-cleanup@1.0.3
herdr plugin link "$PWD/node_modules/@sebastienelet/herdr-worktree-cleanup"
```

## Behaviour

- Only clean linked worktrees are removed; dirty, untracked, locked, foreign and detached checkouts stay intact.
- Deletion preserves the branch. Before removal, ignored files outside disposable directories cause a reported failure and the checkout is kept. An inspection error or warning also refuses removal.
- Ignored files under root-relative `node_modules/`, `target/` and `.moon/cache/` are disposable by default. These exceptions do not permit tracked changes, non-ignored untracked files or removal of locked worktrees. Symlinks or regular files bearing those directory names remain protected.
- Docker cleanup is off by default. With `--docker-cleanup=stop`, only containers matching the exact Compose `working_dir` label are stopped; Docker failures do not block closure.

Set `--disposable-ignored-directories=<JSON array>` in the event's command in `herdr-plugin.toml` to **replace** the defaults, for example:

```toml
  '--disposable-ignored-directories=["node_modules","target",".moon/cache","packages/app/node_modules"]',
```

Use `--disposable-ignored-directories=[]` to protect every ignored file. Entries are literal directory paths relative to the checkout root, without a trailing slash, backslash, empty, `.` or `..` segment; patterns do not expand. A broad entry authorizes deletion of every ignored file beneath it. Missing values, invalid JSON/paths and repeated options refuse removal. Empty directories contain no files to protect.

The inspection and Git removal are separate operations. Stop concurrent writers before closure; use `git worktree lock` while a checkout must remain available, including when it supplies deployed links.

## Release

Version 1.0.3 is prepared here. After merge and successful CI on `main`, dispatch the `Checks and publication` workflow on `main` to publish the patch with npm provenance. Registry publication is a separate manual step.
