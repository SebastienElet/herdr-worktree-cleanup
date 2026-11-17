import { afterEach, expect, test } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import {
  invokePlugin,
  removeTemporaryRepositories,
  repositoryWithLinkedWorktree,
  writeIgnoredFile,
} from "./worktree-test-support.ts";

afterEach(removeTemporaryRepositories);

test.each(["build/cache", "cache with spaces"])(
  "the plugin entry point accepts disposable directory %s",
  (directory) => {
    const worktree = repositoryWithLinkedWorktree();
    writeIgnoredFile(worktree, `${directory}/artifact.txt`, `${directory}/`);

    const result = invokePlugin(worktree, [
      `--disposable-ignored-directories=${JSON.stringify([directory])}`,
    ]);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("removed ");
    expect(existsSync(worktree.checkoutPath)).toBe(false);
  },
);

test.each(["*", ":(glob)**"])(
  "treats disposable directory %s literally",
  (directory) => {
    const worktree = repositoryWithLinkedWorktree();
    const notes = writeIgnoredFile(
      worktree,
      "protected/notes.txt",
      "protected/",
    );

    const result = invokePlugin(worktree, [
      `--disposable-ignored-directories=${JSON.stringify([directory])}`,
    ]);

    expect(result.exitCode).toBe(1);
    expect(result.stdout.toString()).toContain("protected ignored path");
    expect(readFileSync(notes, "utf8")).toBe("fixture contents\n");
  },
);

test.each(["[]", '["build/cache"]'])(
  "the plugin entry point replaces defaults with %s",
  (directories) => {
    const worktree = repositoryWithLinkedWorktree();
    const cache = writeIgnoredFile(
      worktree,
      "node_modules/cache.txt",
      "node_modules/",
    );

    const result = invokePlugin(worktree, [
      `--disposable-ignored-directories=${directories}`,
    ]);

    expect(result.exitCode).toBe(1);
    expect(result.stdout.toString()).toContain("protected ignored path");
    expect(readFileSync(cache, "utf8")).toBe("fixture contents\n");
  },
);

test.each([
  "--disposable-ignored-directories=invalid-json",
  "--disposable-ignored-directories=null",
  '--disposable-ignored-directories=["."]',
  '--disposable-ignored-directories=["../protected"]',
  '--disposable-ignored-directories=["/tmp/cache"]',
  '--disposable-ignored-directories=["cache//nested"]',
  '--disposable-ignored-directories=["cache/../protected"]',
  String.raw`--disposable-ignored-directories=["cache\\nested"]`,
  String.raw`--disposable-ignored-directories=["cache\u0000nested"]`,
  '--disposable-ignored-directories=[""]',
  "--disposable-ignored-directories=[1]",
  "--disposable-ignored-directories",
])(
  "the plugin entry point refuses invalid configuration %s without deletion",
  (argument) => {
    const worktree = repositoryWithLinkedWorktree();

    const result = invokePlugin(worktree, [argument]);

    expect(result.exitCode).toBe(1);
    expect(result.stdout.toString()).toContain(
      "failed: invalid disposable ignored directories",
    );
    expect(existsSync(worktree.checkoutPath)).toBe(true);
  },
);

test("the plugin entry point refuses duplicate disposable directory options", () => {
  const worktree = repositoryWithLinkedWorktree();

  const result = invokePlugin(worktree, [
    "--disposable-ignored-directories=[]",
    '--disposable-ignored-directories=["target"]',
  ]);

  expect(result.exitCode).toBe(1);
  expect(result.stdout.toString()).toContain(
    "failed: invalid disposable ignored directories",
  );
  expect(existsSync(worktree.checkoutPath)).toBe(true);
});
