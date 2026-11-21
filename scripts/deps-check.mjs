#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const stampPath = root => resolve(root, 'node_modules/.map-holons-deps.json');
const stampVersion = 2;

// These are the files that determine the root npm workspace installation. The
// package manifests bundled with schema resources are application data, not npm
// manifests, and nested workspace lockfiles do not participate in root npm ci.
const dependencyInputs = [
  '.npmrc',
  'package.json',
  'package-lock.json',
  'happ/package.json',
  'host/package.json',
  'host/map-sdk/package.json',
];

// Installation inputs only: changing application commands or documentation does
// not change the dependency tree. Keep resolution, platform and install policy.
const installationFields = [
  'name', 'version', 'workspaces', 'dependencies', 'devDependencies',
  'optionalDependencies', 'peerDependencies', 'peerDependenciesMeta',
  'bundledDependencies', 'bundleDependencies', 'overrides', 'engines',
  'devEngines', 'packageManager', 'os', 'cpu', 'libc', 'bin', 'directories',
  'installConfig', 'allowScripts',
];
function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.keys(value).sort().map(key => [key, canonical(value[key])]));
  }
  return value;
}
export function installationManifest(manifest) {
  return Object.fromEntries(installationFields.filter(key => key in manifest)
    .map(key => [key, canonical(manifest[key])]));
}
export async function dependencyFingerprint(root = repositoryRoot) {
  const hash = createHash('sha256');

  for (const relativePath of dependencyInputs) {
    let content = await readFile(resolve(root, relativePath));
    if (relativePath.endsWith('package.json')) {
      content = JSON.stringify(installationManifest(JSON.parse(content.toString('utf8'))));
    }
    hash.update(relativePath);
    hash.update('\0');
    hash.update(content);
    hash.update('\0');
  }

  return hash.digest('hex');
}

export async function recordInstalledDependencies(root = repositoryRoot) {
  await mkdir(dirname(stampPath(root)), { recursive: true });
  await writeFile(
    stampPath(root),
    `${JSON.stringify({ version: stampVersion, fingerprint: await dependencyFingerprint(root) })}\n`,
  );
}

export async function checkInstalledDependencies(root = repositoryRoot) {
  if (!existsSync(stampPath(root))) {
    throw new Error('No dependency installation record was found.');
  }

  let stamp;
  try {
    stamp = JSON.parse(await readFile(stampPath(root), 'utf8'));
  } catch {
    throw new Error('The dependency installation record is unreadable.');
  }

  if (stamp.version !== stampVersion || stamp.fingerprint !== (await dependencyFingerprint(root))) {
    throw new Error('Installed dependencies do not match the checked-out npm workspace inputs.');
  }
}

async function main() {
  if (process.argv[2] === '--record') {
    await recordInstalledDependencies();
    return;
  }
  try {
    await checkInstalledDependencies();
    console.log('npm dependencies are current.');
  } catch (error) {
    console.error(`npm dependencies need installation: ${error.message}`);
    console.error('Run `npm ci` from the repository root, then retry.');
    process.exitCode = 1;
  }
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  await main();
}
