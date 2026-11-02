import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, copyFileSync, writeFileSync, chmodSync, rmSync, unlinkSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';

const root = new URL('../', import.meta.url);
const supported = process.platform === 'darwin' && process.arch === 'arm64';
function isolated(t, native) {
  const directory = mkdtempSync(join(tmpdir(), 'cleanup launcher '));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  mkdirSync(join(directory, 'bin'));
  writeFileSync(join(directory, 'package.json'), JSON.stringify({ version: '9.8.7-test' }));
  copyFileSync(new URL('bin/herdr-worktree-cleanup.mjs', root), join(directory, 'bin/cleanup.mjs'));
  if (native) {
    const platform = join(directory, 'node_modules/@sebastienelet/herdr-worktree-cleanup-darwin-arm64');
    mkdirSync(join(platform, 'bin'), { recursive: true });
    writeFileSync(join(platform, 'package.json'), JSON.stringify({ version: '9.8.7-test' }));
    const executable = join(platform, 'bin/herdr-worktree-cleanup');
    writeFileSync(executable, native);
    chmodSync(executable, 0o755);
  }
  return directory;
}
function launch(directory, args = []) {
  return spawnSync(process.execPath, [join(directory, 'bin/cleanup.mjs'), ...args], { encoding: 'utf8', env: { ...process.env, HERDR_PLUGIN_EVENT_JSON: 'synthetic event' } });
}

test('missing native package fails explicitly and reads the entry package version', t => {
  const result = launch(isolated(t));
  assert.equal(result.status, 1);
  assert.equal(result.stdout, '');
  assert.match(result.stderr, supported ? /Missing @sebastienelet\/herdr-worktree-cleanup-darwin-arm64@9\.8\.7-test/ : /Unsupported platform/);
});

test('launcher preserves arguments, environment, cwd, stdout and exit status', t => {
  const directory = isolated(t, '#!/bin/sh\nprintf "%s\\n" "$HERDR_PLUGIN_EVENT_JSON" "$PWD" "$@"\nexit 7\n');
  const result = launch(directory, ['--docker-cleanup=stop', 'a b']);
  if (!supported) { assert.equal(result.status, 1); assert.match(result.stderr, /Unsupported platform/); return; }
  assert.equal(result.status, 7);
  assert.equal(result.stdout, `synthetic event\n${process.cwd()}\n--docker-cleanup=stop\na b\n`);
  assert.equal(result.stderr, '');
});

test('launcher propagates native signals', t => {
  const result = launch(isolated(t, '#!/bin/sh\nkill -TERM $$\n'));
  if (!supported) { assert.equal(result.status, 1); assert.match(result.stderr, /Unsupported platform/); return; }
  assert.equal(result.signal, 'SIGTERM');
  assert.equal(result.stdout, '');
  assert.equal(result.stderr, '');
});


test('a missing or non-executable native binary fails without fallback', t => {
  for (const missing of [true, false]) {
    const directory = isolated(t, '#!/bin/sh\nexit 0\n');
    const executable = join(directory, 'node_modules/@sebastienelet/herdr-worktree-cleanup-darwin-arm64/bin/herdr-worktree-cleanup');
    if (missing) unlinkSync(executable);
    else chmodSync(executable, 0o644);
    const result = launch(directory);
    assert.equal(result.status, 1);
    assert.equal(result.stdout, '');
    assert.match(result.stderr, supported ? /ENOENT|EACCES/ : /Unsupported platform/);
  }
});

if (supported) test('packed npm packages install offline and execute the native handler through both entry points', t => {
  const directory = mkdtempSync(join(tmpdir(), 'cleanup npm '));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  function run(executable, args, options = {}) {
    const result = spawnSync(executable, args, { encoding: 'utf8', cwd: directory, timeout: 30000, ...options });
    assert.equal(result.error, undefined, result.error?.message);
    assert.equal(result.status, 0, result.stderr);
    return result.stdout;
  }
  const native = JSON.parse(run('npm', ['pack', new URL('packages/darwin-arm64', root).pathname, '--ignore-scripts', '--json', '--pack-destination', directory]))[0].filename;
  const entry = JSON.parse(run('npm', ['pack', root.pathname, '--ignore-scripts', '--json', '--pack-destination', directory]))[0].filename;
  run('npm', ['install', '--offline', '--ignore-scripts', '--package-lock=false', '--no-audit', '--no-fund', join(directory, native), join(directory, entry)]);
  const env = { ...process.env, HERDR_PLUGIN_EVENT_JSON: '{"data":{}}' };
  assert.equal(run(join(directory, 'node_modules/.bin/herdr-worktree-cleanup'), [], { env }), 'skipped: event carries no worktree snapshot\n');
  const nativeDirectory = join(directory, 'node_modules/@sebastienelet/herdr-worktree-cleanup-darwin-arm64');
  assert.equal(run('bin/herdr-worktree-cleanup', ['--docker-cleanup=off'], { env, cwd: nativeDirectory }), 'skipped: event carries no worktree snapshot\n');
  const failure = spawnSync(join(directory, 'node_modules/.bin/herdr-worktree-cleanup'), [], { encoding: 'utf8', env: { ...env, HERDR_PLUGIN_EVENT_JSON: '' } });
  assert.equal(failure.status, 1);
  assert.equal(failure.stdout, 'failed: JSON Parse error: Unexpected EOF\n');
  assert.equal(failure.stderr, '');
});
