import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const repository = resolve(process.cwd(), '..');
const implementations = [
  ['PathInspectorTypeScript.VisualizerImplementation', 'path-inspector.js'],
  ['HolonInspectorTypeScript.VisualizerImplementation', 'holon-inspector.js'],
  ['TableCollectionTypeScript.VisualizerImplementation', 'table-collection.js'],
];

describe('registered navigation artifact integrity', () => {
  it.each(implementations)('%s matches the executable bytes in source and packaged resources', async (key, filename) => {
    const artifact = await readFile(resolve(repository, 'host/conductora/resources/dahn-visualizers', filename));
    const digest = `sha256:${createHash('sha256').update(artifact).digest('hex')}`;
    const houseTroupe = filename === 'table-collection.js';
    const source = await readFile(resolve(repository, houseTroupe ? 'house-troupe/space-navigator/schema/schema.tdl' : 'schema-src/dahn/schema.tdl'), 'utf8');
    const declaration = source.split(`instance ${key} {`)[1]?.split('\n}')[0];
    expect(declaration).toContain(`VisualizerArtifactDigest "${digest}"`);
    for (const path of houseTroupe ? [
      'generated/house-troupe/space-navigator/imports/schema.json',
      'host/conductora/resources/house-troupe/space-navigator/imports/schema.json',
    ] : [
      'generated/json-imports/dahn/schema.json',
      'generated/core-schema-bootstrap/imports/dahn/schema.json',
      'host/conductora/resources/core-schema-bootstrap/imports/dahn/schema.json',
    ]) {
      const generated = await readFile(resolve(repository, path), 'utf8');
      const holons = JSON.parse(generated).holons as Array<{ key: string; properties: Record<string, unknown> }>;
      expect(holons.find(holon => holon.key === key)?.properties['VisualizerArtifactDigest']).toBe(digest);
    }
  });
});
