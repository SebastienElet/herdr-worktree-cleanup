# Herdr Worktree Cleanup

Native Rust handler for Herdr `workspace.closed`, plugin ID `dotfiles.worktree-cleanup`.
Git refuses dirty, untracked, locked and foreign checkouts. Detached HEADs and branches are
preserved. Ignored files are protected except under the root-relative directories
`node_modules/`, `target/` and `.moon/cache/`. A Git inspection error or warning also
prevents removal. Files and symlinks named like these directories remain protected.

Set `--disposable-ignored-directories=<JSON array>` in the event command to replace the defaults.

```toml
command = [
  "bin/herdr-worktree-cleanup",
  "--docker-cleanup=off",
  '--disposable-ignored-directories=["node_modules","target",".moon/cache","packages/app/node_modules"]',
]
```

Use `--disposable-ignored-directories=[]` to protect every ignored file. Entries are literal
root-relative directory paths. Empty, dot and parent segments, trailing slashes, backslashes,
missing values and repeated options are rejected. A broad entry makes every ignored file
under that directory disposable. Git still refuses tracked changes and non-ignored untracked
files inside these directories.

The handler requires an absolute checkout path and clears inherited repository-local Git
overrides before inspecting or removing it. Inspection and removal are separate Git operations. Stop concurrent writers before closure
and lock worktrees that must stay available, including those supplying deployed links.

Target: macOS ARM64 with Git. Herdr 0.9.3 is the declared minimum.
Docker is optional and off by default. With `--docker-cleanup=stop`, the handler stops
containers matching the exact Compose `working_dir` label after checking HEAD and ignored
files. Docker failures and 15-second command timeouts do not block Git cleanup.

Accepted parity exception: malformed `length` array coercion is limited to 4,096 nesting levels.
Beyond that, the handler returns `failed: Maximum call stack size exceeded.` with exit code 1.
This deterministic limit replaces Bun's runtime-dependent stack limit; JSON parsing is not capped by it.

After publication, install in a durable directory:

```sh
mkdir -p "$HOME/.local/share/herdr-worktree-cleanup"
cd "$HOME/.local/share/herdr-worktree-cleanup"
npm install --save-exact --ignore-scripts @sebastienelet/herdr-worktree-cleanup@1.0.4
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

Automated tests use disposable Git repositories and substitute Docker. A separate smoke test on
macOS ARM64 used isolated Herdr 0.9.3 and 0.9.5 servers to exercise clean removal and refusal
when ignored data was protected.

The manual `main` workflow publishes the native package, then the launcher, with npm
provenance. The first native release requires registering the package and configuring its
trusted publisher before running the workflow.
See [distribution decision](docs/adr/0001-native-rust-distribution.md).
