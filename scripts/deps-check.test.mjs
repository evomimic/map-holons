import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { recordInstalledDependencies, checkInstalledDependencies } from './deps-check.mjs';

async function installation(t) {
  const root = await mkdtemp(join(tmpdir(), 'map-deps-check-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  for (const name of ['package.json', 'happ/package.json', 'host/package.json', 'host/map-sdk/package.json']) {
    await mkdir(dirname(join(root, name)), { recursive: true });
    await writeFile(join(root, name), JSON.stringify({ name, version: '1.0.0', dependencies: { alpha: '1.0.0' } }));
  }
  await writeFile(join(root, '.npmrc'), 'engine-strict=true\n');
  await writeFile(join(root, 'package-lock.json'), '{"lockfileVersion":3}');
  await recordInstalledDependencies(root);
  return root;
}
async function edit(root, path, mutate) {
  const file = join(root, path);
  const data = JSON.parse(await readFile(file, 'utf8'));
  mutate(data);
  await writeFile(file, JSON.stringify(data, null, 2));
}

test('script, description and formatting changes leave installed dependencies current', async t => {
  const root = await installation(t);
  for (const path of ['package.json', 'happ/package.json', 'host/package.json', 'host/map-sdk/package.json']) {
    await edit(root, path, data => { data.scripts = { start: 'different launch' }; data.description = 'Changed'; });
  }
  await checkInstalledDependencies(root);
});

for (const [field, value] of Object.entries({
  dependencies: { alpha: '2.0.0' }, devDependencies: { beta: '1.0.0' },
  optionalDependencies: { beta: '1.0.0' }, peerDependencies: { beta: '^1.0.0' },
  peerDependenciesMeta: { beta: { optional: true } }, overrides: { alpha: '2.0.0' },
  workspaces: ['packages/*'], engines: { node: '>=24' }, devEngines: { runtime: { name: 'node', version: '>=24' } },
  packageManager: 'npm@11.0.0', os: ['linux'], cpu: ['arm64'], libc: ['glibc'],
  bin: { tool: 'cli.js' }, bundledDependencies: ['alpha'], bundleDependencies: ['alpha'],
  allowScripts: { alpha: false }, installConfig: { hoistingLimits: 'workspaces' },
})) {
  test(`detects ${field} changes`, async t => {
    const root = await installation(t);
    await edit(root, 'package.json', data => { data[field] = value; });
    await assert.rejects(checkInstalledDependencies(root), /do not match/);
  });
}
for (const path of ['happ/package.json', 'host/package.json', 'host/map-sdk/package.json']) {
  test(`detects dependency changes in ${path}`, async t => {
    const root = await installation(t);
    await edit(root, path, data => { data.dependencies.alpha = '2.0.0'; });
    await assert.rejects(checkInstalledDependencies(root), /do not match/);
  });
}
for (const path of ['.npmrc', 'package-lock.json']) {
  test(`detects changes to ${path}`, async t => {
    const root = await installation(t);
    await writeFile(join(root, path), 'changed');
    await assert.rejects(checkInstalledDependencies(root), /do not match/);
  });
}
test('dependency map key order does not require reinstallation', async t => {
  const root = await installation(t);
  await edit(root, 'package.json', data => { data.dependencies = { alpha: '1.0.0', beta: '2.0.0' }; });
  await recordInstalledDependencies(root);
  await edit(root, 'package.json', data => { data.dependencies = { beta: '2.0.0', alpha: '1.0.0' }; });
  await checkInstalledDependencies(root);
});
test('missing, invalid and old installation records fail closed', async t => {
  const root = await installation(t);
  const stamp = join(root, 'node_modules/.map-holons-deps.json');
  await rm(stamp);
  await assert.rejects(checkInstalledDependencies(root), /No dependency/);
  await writeFile(stamp, 'invalid');
  await assert.rejects(checkInstalledDependencies(root), /unreadable/);
  await writeFile(stamp, JSON.stringify({ version: 1, fingerprint: 'old' }));
  await assert.rejects(checkInstalledDependencies(root), /do not match/);
});
