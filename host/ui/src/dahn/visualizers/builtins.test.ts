import { describe, expect, it } from 'vitest';
import { DefaultVisualizerRegistry } from '../registry/default-visualizer-registry';
import { registerBuiltInVisualizers } from '../registry/register-builtins';

describe('registerBuiltInVisualizers', () => {
  it('registers the phase 0 built-in visualizer definitions', () => {
    const registry = new DefaultVisualizerRegistry();

    registerBuiltInVisualizers(registry);

    expect(
      registry.getByImplementationKey('dahn.space-navigator'),
    ).toMatchObject({ id: 'space-navigator' });
    expect(
      registry.getByImplementationKey('dahn.generic-holon-node'),
    ).toMatchObject({ id: 'holon-node' });
    expect(
      registry.getByImplementationKey('dahn.table-collection'),
    ).toMatchObject({ id: 'table-collection' });
    expect(registry.get('action-menu')).toBeDefined();
    expect(registry.get('debug')).toBeDefined();
    expect(registry.list()).toHaveLength(5);
  });
});
