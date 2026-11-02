import { mkdirSync, copyFileSync, chmodSync, readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';

const root = fileURLToPath(new URL('../', import.meta.url));
const manifest = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
const platform = `${process.platform}-${process.arch}`;
if (platform !== 'darwin-arm64') throw new Error(`Unsupported build platform: ${platform}`);
const debug = process.argv.includes('--debug');
execFileSync('cargo', ['build', '--locked', ...(debug ? [] : ['--release'])], { cwd: root, stdio: 'inherit' });
const source = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim();
const dirty = execFileSync('git', ['status', '--porcelain'], { cwd: root, encoding: 'utf8' }).length > 0;
const directory = join(root, 'packages', platform);
mkdirSync(join(directory, 'bin'), { recursive: true });
const binary = join(directory, 'bin/herdr-worktree-cleanup');
copyFileSync(join(root, 'target', debug ? 'debug' : 'release', 'herdr-worktree-cleanup'), binary);
chmodSync(binary, 0o755);
copyFileSync(join(root, 'LICENSE'), join(directory, 'LICENSE'));
copyFileSync(join(root, 'herdr-plugin.toml'), join(directory, 'herdr-plugin.toml'));
writeFileSync(join(directory, 'package.json'), JSON.stringify({
  name: `${manifest.name}-${platform}`,
  version: manifest.version,
  description: `Native Herdr worktree cleanup for ${platform}`,
  license: manifest.license,
  repository: manifest.repository,
  os: ['darwin'],
  cpu: ['arm64'],
  files: ['bin', 'herdr-plugin.toml', 'BUILD.json', 'LICENSE'],
  publishConfig: manifest.publishConfig,
}, null, 2) + '\n');
const sha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
writeFileSync(join(directory, 'BUILD.json'), JSON.stringify({ version: manifest.version, source, dirty, platform, sha256 }, null, 2) + '\n');
