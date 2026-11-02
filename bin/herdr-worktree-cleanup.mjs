#!/usr/bin/env node
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { spawnSync } from 'node:child_process';

try {
  const manifest = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));
  const platform = `${process.platform}-${process.arch}`;
  if (platform !== 'darwin-arm64') throw new Error(`Unsupported platform: ${platform}`);
  const name = `${manifest.name ?? '@sebastienelet/herdr-worktree-cleanup'}-${platform}`;
  let packagePath;
  try {
    packagePath = createRequire(import.meta.url).resolve(`${name}/package.json`);
  } catch {
    throw new Error(`Missing ${name}@${manifest.version}; install optional dependencies with npm`);
  }
  const executable = join(dirname(packagePath), 'bin/herdr-worktree-cleanup');
  const result = spawnSync(executable, process.argv.slice(2), { stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.signal) process.kill(process.pid, result.signal);
  process.exit(result.status ?? 1);
} catch (error) {
  console.error(`herdr-worktree-cleanup: ${error.message}`);
  process.exit(1);
}
