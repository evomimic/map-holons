import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const repository = resolve(process.cwd(), '..');
const implementations = [
  ['LoadHolonsInspectorTypeScript.VisualizerImplementation', 'load-holons-inspector.js'],
  ['PathInspectorTypeScript.VisualizerImplementation', 'path-inspector.js'],
  ['HolonInspectorTypeScript.VisualizerImplementation', 'holon-inspector.js'],
  ['ConnectionsFirstInspectorTypeScript.VisualizerImplementation', 'connections-first-inspector.js'],
  ['VisualizerInspectorTypeScript.VisualizerImplementation', 'visualizer-inspector.js'],
  ['DiscoveryTreeTypeScript.VisualizerImplementation', 'discovery-tree.js'],
  ['DiscoveryLevelsTypeScript.VisualizerImplementation', 'discovery-levels.js'],
  ['TableCollectionTypeScript.VisualizerImplementation', 'table-collection.js'],
  ['GenericActionsTypeScript.VisualizerImplementation', 'actions.js'],
  ['LoadHolonsActionTypeScript.VisualizerImplementation', 'load-holons-action.js'],
  ['UnsupportedActionTypeScript.VisualizerImplementation', 'unsupported-action.js'],
  ['DefaultPropertyMapVisualizerTypeScript.VisualizerImplementation', 'properties.js'],
  ['GenericPropertyTypeScript.VisualizerImplementation', 'property.js'],
  ['ScalarValueTypeScript.VisualizerImplementation', 'scalar-value.js'],
];

describe('registered navigation artifact integrity', () => {
  it.each(implementations)('%s matches the executable bytes in source and packaged resources', async (key, filename) => {
    const artifact = await readFile(resolve(repository, 'host/conductora/resources/dahn-visualizers', filename));
    const digest = `sha256:${createHash('sha256').update(artifact).digest('hex')}`;
    const houseTroupe = ['table-collection.js', 'visualizer-inspector.js', 'discovery-tree.js', 'discovery-levels.js'].includes(filename);
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

it('offers rooted navigation at every TypeKind boundary supported by the generic Node', async () => {
  const generated = JSON.parse(await readFile(resolve(repository, 'generated/json-imports/dahn/schema.json'), 'utf8'));
  const applicability = (key: string): string[] => generated.holons.find((h: any) => h.key === key)
    .relationships.find((r: any) => r.name === 'ApplicableToType').target.map((t: any) => t.$ref);
  const roots = applicability('PathInspector.RootedNavigationVisualizer');
  for (const type of applicability('HolonInspector.NodeVisualizer')) expect(roots).toContain(type);
});

it('gives each local Node alternative independent child slots with exactly one Visualizer owner', async () => {
  const generated = JSON.parse(await readFile(resolve(repository, 'generated/json-imports/dahn/schema.json'), 'utf8'));
  const targets = (holon: any, relation: string): string[] => holon.relationships?.find((item: any) => item.name === relation)?.target.map((item: any) => item.$ref) ?? [];
  const original = targets(generated.holons.find((holon: any) => holon.key === 'HolonInspector.NodeVisualizer'), 'HasSlot');
  const alternate = targets(generated.holons.find((holon: any) => holon.key === 'ConnectionsFirstInspector.NodeVisualizer'), 'HasSlot');
  expect(alternate).toHaveLength(original.length);
  for (const slot of [...original, ...alternate]) {
    expect(generated.holons.filter((holon: any) => targets(holon, 'HasSlot').includes(slot))).toHaveLength(1);
  }
  expect(alternate.some(slot => original.includes(slot))).toBe(false);
});
