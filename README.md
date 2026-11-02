# Herdr Worktree Cleanup

Native Rust handler for Herdr `workspace.closed`, plugin ID `dotfiles.worktree-cleanup`.
Git refuses dirty, untracked, locked and foreign checkouts. Detached HEADs and branches are
preserved. Ignored files can disappear with an otherwise clean checkout: lock deployment sources.

Target: macOS ARM64, Git, Herdr ≥0.9.3 (declared minimum, not qualification of later versions).
Docker is optional: `--docker-cleanup=off` by default; `--docker-cleanup=stop` uses the exact Compose
`working_dir` label. Docker failures and 15-second command timeouts do not block Git cleanup.

Accepted parity exception: malformed `length` array coercion is limited to 4,096 nesting levels.
Beyond that, the handler returns `failed: Maximum call stack size exceeded.` with exit code 1.
This deterministic limit replaces Bun's runtime-dependent stack limit; JSON parsing is not capped by it.

After publication and qualification, install in a durable directory:

```sh
mkdir -p "$HOME/.local/share/herdr-worktree-cleanup"
cd "$HOME/.local/share/herdr-worktree-cleanup"
npm install --save-exact --ignore-scripts @sebastienelet/herdr-worktree-cleanup@1.0.2
herdr plugin link "$PWD/node_modules/@sebastienelet/herdr-worktree-cleanup-darwin-arm64"
```

Keep the installation lockfile; reproduce with `npm ci --ignore-scripts`. The native package
contains the Herdr manifest and binary; the main package exposes the Node ≥22.14.0 CLI launcher.
The following switching procedure remains to be qualified in Herdr. Suspend closures and record
`herdr plugin list`. Disable/unlink only the owned
`dotfiles.worktree-cleanup` registration, then link the native package and test disposable worktrees.
Maintain one active handler. Roll back by disabling/unlinking it and restoring the recorded installation;
deleted ignored files cannot be recovered this way. Uninstall by disabling/unlinking before npm removal.

Development (Rust toolchain pinned in `rust-toolchain.toml`):

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
npm run build
npm test
actionlint
```

Tests use disposable Git repositories and substitute Docker. Native Herdr event delivery and real
Docker are not covered. No npm publication or dotfiles migration is performed by these commands.
The manual `main` workflow is configured to publish native then launcher with OIDC provenance after checks.
See [distribution decision](docs/adr/0001-native-rust-distribution.md).
