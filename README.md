# Herdr Worktree Cleanup

Herdr plugin that removes an eligible linked Git worktree when its workspace closes.

Requires macOS ARM64, Bun 1.4.3 and Herdr 0.9.3 (declared minimum; later versions are unqualified).

## Install

```sh
mkdir -p "$HOME/.local/share/herdr-worktree-cleanup"
cd "$HOME/.local/share/herdr-worktree-cleanup"
bun install --exact --ignore-scripts @sebastienelet/herdr-worktree-cleanup@1.0.2
herdr plugin link "$PWD/node_modules/@sebastienelet/herdr-worktree-cleanup"
```

## Behaviour

- Only clean linked worktrees are removed; dirty, untracked, locked, foreign and detached checkouts stay intact.
- Deletion preserves the branch. Ignored files can be deleted with an otherwise clean checkout.
- Docker cleanup is off by default. With `--docker-cleanup=stop`, only containers matching the exact Compose `working_dir` label are stopped; Docker failures do not block closure.
