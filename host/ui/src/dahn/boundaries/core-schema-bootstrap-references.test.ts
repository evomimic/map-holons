import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { expect, it } from 'vitest';

/** Bootstrap has no prior namespace: every authored target must exist in its selected bundle. */
it('resolves every relationship target in the packaged Core Schema bootstrap bundle', async () => {
  const bundle = resolve(process.cwd(), 'conductora/resources/core-schema-bootstrap');
  const manifest = JSON.parse(await readFile(resolve(bundle, 'manifest.json'), 'utf8'));
  const imports = await Promise.all(manifest.imports.map(async (entry: { path: string }) =>
    JSON.parse(await readFile(resolve(bundle, entry.path), 'utf8'))));
  const holons = imports.flatMap(file => file.holons);
  const keys = new Set(holons.map(holon => holon.key));
  const unresolved: string[] = [];
  for (const holon of holons) {
    for (const relationship of holon.relationships ?? []) {
      for (const target of relationship.target ?? []) {
        if (target.$ref && !keys.has(target.$ref)) {
          unresolved.push(`${holon.key} — ${relationship.name} → ${target.$ref}`);
        }
      }
    }
  }
  expect(unresolved).toEqual([]);
});

it('keeps the loader-specific Node applicable only to load responses', async () => {
  const schema = JSON.parse(await readFile(resolve(process.cwd(), 'conductora/resources/core-schema-bootstrap/imports/dahn/schema.json'), 'utf8'));
  const visualizer = schema.holons.find((holon: { key: string }) => holon.key === 'LoadHolons.NodeVisualizer');
  const applicable = visualizer.relationships.find((relationship: { name: string }) => relationship.name === 'ApplicableToType');
  expect(applicable.target).toEqual([{ $ref: 'HolonLoadResponse.DanceResponseType' }]);
});
