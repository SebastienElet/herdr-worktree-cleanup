import { afterEach, expect, test } from "bun:test";
import {
  appendFileSync,
  chmodSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readlinkSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import {
  git,
  invokePlugin,
  removeTemporaryRepositories,
  repositoryWithLinkedWorktree,
  workspaceClosedEvent,
  writeIgnoredFile,
} from "./worktree-test-support.ts";
import type { RemovalOutcome } from "./remove-closed-worktree.ts";
import { removeClosedWorktree } from "./remove-closed-worktree.ts";

afterEach(removeTemporaryRepositories);

test("fails and preserves ignored files in an otherwise clean checkout", () => {
  const { checkoutPath, repositoryRoot } = repositoryWithLinkedWorktree();
  git(repositoryRoot, "config", "status.showUntrackedFiles", "no");
  writeFileSync(
    join(repositoryRoot, ".git", "info", "exclude"),
    "ignored-notes.txt\n",
  );
  writeFileSync(
    join(checkoutPath, "ignored-notes.txt"),
    "protected ignored notes\n",
  );

  const outcome = removeClosedWorktree(
    workspaceClosedEvent({
      checkoutPath,
      isLinkedWorktree: true,
      repositoryRoot,
    }),
  );

  expect(outcome.kind).toBe("failed");
  expect(outcome.kind === "failed" && outcome.reason).toContain(
    "ignored-notes.txt",
  );
  expect(readFileSync(join(checkoutPath, "ignored-notes.txt"), "utf8")).toBe(
    "protected ignored notes\n",
  );
});

test.each(["node_modules", "target", ".moon/cache"])(
  "removes an ignored disposable %s directory and keeps its branch",
  (directory) => {
    const worktree = repositoryWithLinkedWorktree();
    writeIgnoredFile(worktree, `${directory}/cache.txt`, `${directory}/`);

    const outcome = removeClosedWorktree(
      workspaceClosedEvent({ ...worktree, isLinkedWorktree: true }),
    );

    expect(outcome).toEqual({
      kind: "removed",
      checkoutPath: worktree.checkoutPath,
    });
    expect(existsSync(worktree.checkoutPath)).toBe(false);
    expect(
      git(worktree.repositoryRoot, "branch", "--list", "feature"),
    ).toContain("feature");
  },
);

test.each([
  "protected/notes.txt",
  "node_modules-backup/notes.txt",
  "target-backup/notes.txt",
  ".moon/notes.txt",
  "packages/app/node_modules/notes.txt",
  "protected/notes \nü.txt",
  "node_modules",
  "target",
  ".moon/cache",
])("preserves protected ignored path %j", (relativePath) => {
  const worktree = repositoryWithLinkedWorktree();
  const path = writeIgnoredFile(worktree, relativePath, "*");

  const outcome = removeClosedWorktree(
    workspaceClosedEvent({ ...worktree, isLinkedWorktree: true }),
  );

  expect(outcome.kind).toBe("failed");
  expect(readFileSync(path, "utf8")).toBe("fixture contents\n");
});

test("preserves protected ignored data alongside disposable caches", () => {
  const worktree = repositoryWithLinkedWorktree();
  const cache = writeIgnoredFile(
    worktree,
    "node_modules/cache.txt",
    "node_modules/",
  );
  const notes = writeIgnoredFile(worktree, "protected/notes.txt", "protected/");

  const outcome = removeClosedWorktree(
    workspaceClosedEvent({ ...worktree, isLinkedWorktree: true }),
  );

  expect(outcome.kind).toBe("failed");
  expect(readFileSync(cache, "utf8")).toBe("fixture contents\n");
  expect(readFileSync(notes, "utf8")).toBe("fixture contents\n");
});

test("preserves a symlink with a disposable directory name and its external target", () => {
  const worktree = repositoryWithLinkedWorktree();
  const target = join(worktree.repositoryRoot, "notes.txt");
  writeFileSync(target, "external notes\n");
  const link = join(worktree.checkoutPath, "node_modules");
  symlinkSync(target, link);
  appendFileSync(
    join(worktree.repositoryRoot, ".git", "info", "exclude"),
    "node_modules\n",
  );

  const outcome = removeClosedWorktree(
    workspaceClosedEvent({ ...worktree, isLinkedWorktree: true }),
  );

  expect(outcome.kind).toBe("failed");
  expect(readlinkSync(link)).toBe(target);
  expect(readFileSync(target, "utf8")).toBe("external notes\n");
});

test("fails without deletion when Git cannot inspect ignored files", () => {
  const worktree = repositoryWithLinkedWorktree();
  const notes = writeIgnoredFile(worktree, "notes.txt");
  git(
    worktree.repositoryRoot,
    "config",
    "core.excludesFile",
    worktree.repositoryRoot,
  );

  const outcome = removeClosedWorktree(
    workspaceClosedEvent({ ...worktree, isLinkedWorktree: true }),
  );

  expect(outcome.kind).toBe("failed");
  expect(outcome.kind === "failed" && outcome.reason).toContain(
    "cannot inspect ignored files",
  );
  expect(readFileSync(notes, "utf8")).toBe("fixture contents\n");
});

test("fails without deletion when Git cannot read an ignored directory", () => {
  const worktree = repositoryWithLinkedWorktree();
  const notes = writeIgnoredFile(worktree, "protected/notes.txt", "protected/");
  const directory = dirname(notes);
  const permissions = statSync(directory).mode;
  chmodSync(directory, 0);

  const outcome = ((): RemovalOutcome => {
    try {
      return removeClosedWorktree(
        workspaceClosedEvent({ ...worktree, isLinkedWorktree: true }),
      );
    } finally {
      chmodSync(directory, permissions);
    }
  })();

  expect(outcome.kind).toBe("failed");
  expect(outcome.kind === "failed" && outcome.reason).toContain(
    "cannot inspect ignored files",
  );
  expect(readFileSync(notes, "utf8")).toBe("fixture contents\n");
});

test("the plugin entry point reports protected ignored files with a failing exit", () => {
  const worktree = repositoryWithLinkedWorktree();
  const notes = writeIgnoredFile(worktree, "protected/notes.txt", "protected/");

  const result = invokePlugin(worktree);

  expect(result.exitCode).toBe(1);
  expect(result.stdout.toString()).toContain("failed: protected ignored path");
  expect(readFileSync(notes, "utf8")).toBe("fixture contents\n");
});

test.each(["repository", "global"])(
  "preserves files ignored by %s rules",
  (source) => {
    const worktree = repositoryWithLinkedWorktree();
    const ignoreFile =
      source === "repository"
        ? join(worktree.checkoutPath, ".gitignore")
        : join(worktree.repositoryRoot, "global-ignore");
    writeFileSync(ignoreFile, "protected/\n");
    if (source === "repository") {
      git(worktree.checkoutPath, "add", ".gitignore");
      git(worktree.checkoutPath, "commit", "-qm", "ignore fixture data");
    } else {
      git(worktree.repositoryRoot, "config", "core.excludesFile", ignoreFile);
    }
    const notes = join(worktree.checkoutPath, "protected", "notes.txt");
    mkdirSync(dirname(notes));
    writeFileSync(notes, "protected contents\n");
    git(worktree.checkoutPath, "check-ignore", "protected/notes.txt");

    const result = invokePlugin(worktree);

    expect(result.exitCode).toBe(1);
    expect(result.stdout.toString()).toContain("protected ignored path");
    expect(readFileSync(notes, "utf8")).toBe("protected contents\n");
  },
);
