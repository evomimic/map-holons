import { describe, expect, it } from 'vitest';
import { DefaultVisualizerRegistry } from '../registry/default-visualizer-registry';
import { registerBuiltInVisualizers } from '../registry/register-builtins';

describe('registerBuiltInVisualizers', () => {
  it('registers the phase 0 built-in visualizer definitions', () => {
    const registry = new DefaultVisualizerRegistry();

    registerBuiltInVisualizers(registry);

    expect(registry.get('holon-node')).toBeDefined();
    expect(registry.get('action-menu')).toBeDefined();
    expect(registry.get('debug')).toBeDefined();
    expect(registry.list()).toHaveLength(3);
  });
});
