#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const stampPath = resolve(repositoryRoot, 'node_modules/.map-holons-deps.json');

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

async function dependencyFingerprint() {
  const hash = createHash('sha256');

  for (const relativePath of dependencyInputs) {
    const content = await readFile(resolve(repositoryRoot, relativePath));
    hash.update(relativePath);
    hash.update('\0');
    hash.update(content);
    hash.update('\0');
  }

  return hash.digest('hex');
}

async function recordInstalledDependencies() {
  await mkdir(dirname(stampPath), { recursive: true });
  await writeFile(
    stampPath,
    `${JSON.stringify({ version: 1, fingerprint: await dependencyFingerprint() })}\n`,
  );
}

async function checkInstalledDependencies() {
  if (!existsSync(stampPath)) {
    throw new Error('No dependency installation record was found.');
  }

  let stamp;
  try {
    stamp = JSON.parse(await readFile(stampPath, 'utf8'));
  } catch {
    throw new Error('The dependency installation record is unreadable.');
  }

  if (stamp.version !== 1 || stamp.fingerprint !== (await dependencyFingerprint())) {
    throw new Error('Installed dependencies do not match the checked-out npm workspace inputs.');
  }
}

if (process.argv[2] === '--record') {
  await recordInstalledDependencies();
  process.exit(0);
}

try {
  await checkInstalledDependencies();
  console.log('npm dependencies are current.');
} catch (error) {
  console.error(`npm dependencies need installation: ${error.message}`);
  console.error('Run `npm ci` from the repository root, then retry.');
  process.exitCode = 1;
}
