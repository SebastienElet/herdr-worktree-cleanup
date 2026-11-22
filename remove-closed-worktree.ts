import { cleanupWorktreeContainers } from "./cleanup-containers.ts";
import { existsSync } from "node:fs";
import { z } from "zod";

const workspaceClosedEventSchema = z.object({
  data: z.object({
    workspace: z
      .object({
        worktree: z
          .object({
            checkout_path: z.string().min(1),
            is_linked_worktree: z.boolean(),
            repo_root: z.string().min(1),
          })
          .optional(),
      })
      .optional(),
  }),
});

type WorkspaceClosedEvent = z.infer<typeof workspaceClosedEventSchema>;

type RemovalOutcome =
  | Readonly<{ kind: "removed"; checkoutPath: string }>
  | Readonly<{ kind: "skipped"; reason: string }>
  | Readonly<{ kind: "failed"; reason: string }>;

const defaultDisposableIgnoredDirectories = [
  "node_modules",
  "target",
  ".moon/cache",
];
const disposableIgnoredDirectoriesSchema = z.array(
  z
    .string()
    .refine(
      (directory) =>
        !directory.includes("\\") &&
        !directory.includes("\0") &&
        directory
          .split("/")
          .every((segment) => !["", ".", ".."].includes(segment)),
      "expected a relative directory without empty, dot or parent segments",
    ),
);

function runGit(
  directory: string,
  arguments_: readonly string[],
): Readonly<{ exitCode: number; stderr: string; stdout: string }> {
  const result = Bun.spawnSync(["git", "-C", directory, ...arguments_], {
    stderr: "pipe",
    stdout: "pipe",
  });
  return {
    exitCode: result.exitCode,
    stderr: result.stderr.toString().trim(),
    stdout: result.stdout.toString(),
  };
}

function ignoredFileProtectionFailure(
  checkoutPath: string,
  disposableIgnoredDirectories: readonly string[],
): string | undefined {
  const ignored = runGit(checkoutPath, [
    "ls-files",
    "--others",
    "--ignored",
    "--exclude-standard",
    "-z",
    "--",
    ".",
    ...disposableIgnoredDirectories.map(
      (directory) => `:(top,exclude,literal)${directory}/`,
    ),
  ]);
  if (ignored.exitCode !== 0 || ignored.stderr !== "") {
    return `cannot inspect ignored files: ${ignored.stderr}`;
  }
  if (ignored.stdout !== "" && !ignored.stdout.endsWith("\0")) {
    return "cannot inspect ignored files: incomplete Git output";
  }
  const protectedPath = ignored.stdout.split("\0").find((path) => path !== "");
  return protectedPath === undefined
    ? undefined
    : `protected ignored path ${JSON.stringify(protectedPath)}; keep the worktree or explicitly declare its directory disposable`;
}

function skipped(reason: string): RemovalOutcome {
  return { kind: "skipped", reason };
}

function parseWorkspaceClosedEvent(
  eventJson: string,
):
  | Readonly<{ ok: true; event: WorkspaceClosedEvent }>
  | Readonly<{ ok: false; reason: string }> {
  try {
    const parsed = workspaceClosedEventSchema.safeParse(JSON.parse(eventJson));
    return parsed.success
      ? { ok: true, event: parsed.data }
      : { ok: false, reason: parsed.error.message };
  } catch (error) {
    return {
      ok: false,
      reason: error instanceof Error ? error.message : String(error),
    };
  }
}

function removeClosedWorktree(
  eventJson: string,
  dockerMode: "off" | "stop" = "off",
  disposableIgnoredDirectories: readonly string[] = defaultDisposableIgnoredDirectories,
): RemovalOutcome {
  const parsed = parseWorkspaceClosedEvent(eventJson);
  if (!parsed.ok) {
    return { kind: "failed", reason: parsed.reason };
  }
  const worktree = parsed.event.data.workspace?.worktree;
  if (worktree === undefined) {
    return skipped("event carries no worktree snapshot");
  }
  if (!worktree.is_linked_worktree) {
    return skipped("workspace is not a linked worktree");
  }
  const checkoutPath = worktree.checkout_path;
  if (!existsSync(checkoutPath)) {
    return skipped("checkout no longer exists");
  }
  if (
    runGit(checkoutPath, ["symbolic-ref", "--quiet", "HEAD"]).exitCode !== 0
  ) {
    return skipped("checkout HEAD is detached and its commits would be lost");
  }
  const protectionFailure = ignoredFileProtectionFailure(
    checkoutPath,
    disposableIgnoredDirectories,
  );
  if (protectionFailure !== undefined) {
    return { kind: "failed", reason: protectionFailure };
  }
  cleanupWorktreeContainers(checkoutPath, dockerMode);
  const removal = runGit(worktree.repo_root, [
    "-c",
    "status.showUntrackedFiles=all",
    "worktree",
    "remove",
    checkoutPath,
  ]);
  return removal.exitCode === 0
    ? { kind: "removed", checkoutPath }
    : { kind: "failed", reason: removal.stderr };
}

function removeFromCommandLine(): RemovalOutcome {
  const optionName = "--disposable-ignored-directories=";
  const options = Bun.argv.filter((argument) =>
    argument.startsWith("--disposable-ignored-directories"),
  );
  const [option] = options;
  if (
    options.length > 1 ||
    (option !== undefined && !option.startsWith(optionName))
  ) {
    return {
      kind: "failed",
      reason:
        "invalid disposable ignored directories: use one JSON array option",
    };
  }
  let directories = defaultDisposableIgnoredDirectories;
  try {
    if (option !== undefined) {
      directories = disposableIgnoredDirectoriesSchema.parse(
        JSON.parse(option.slice(optionName.length)),
      );
    }
  } catch (error) {
    return {
      kind: "failed",
      reason: `invalid disposable ignored directories: ${error instanceof Error ? error.message : String(error)}`,
    };
  }
  return removeClosedWorktree(
    process.env.HERDR_PLUGIN_EVENT_JSON ?? "",
    Bun.argv.includes("--docker-cleanup=stop") ? "stop" : "off",
    directories,
  );
}

if (import.meta.main) {
  const outcome = removeFromCommandLine();
  const message =
    outcome.kind === "removed"
      ? `removed ${outcome.checkoutPath}`
      : `${outcome.kind}: ${outcome.reason}`;
  process.stdout.write(`${message}\n`);
  process.exitCode = outcome.kind === "failed" ? 1 : 0;
}

export { removeClosedWorktree };
export type { RemovalOutcome };
