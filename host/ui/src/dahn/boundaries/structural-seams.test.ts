import { describe, expect, it } from 'vitest';
import { DefaultVisualizerRegistry } from '../registry/default-visualizer-registry';
import { applyTheme } from '../themes/apply-theme';
import { DEFAULT_DAHN_THEME } from '../themes/default-theme';
import type {
  VisualizerDefinition,
} from '../index';

function visualizer(id: string): VisualizerDefinition {
  return {
    id,
    displayName: id,
    version: '0.0.0',
    componentTag: `test-${id}`,
    supportedTargets: [
      { kind: (id === 'action-menu' ? 'action' : 'holon-node') as const },
    ],
    load: async () => {},
  };
}

describe('DAHN structural seams', () => {
  it('keeps the registry as an id-keyed activation registry, not a selector', () => {
    const registry = new DefaultVisualizerRegistry();
    const definition = visualizer('debug');

    registry.register(definition);

    expect(registry.get('debug')).toBe(definition);
    expect(registry.list()).toEqual([definition]);
  });

  it('keeps theme application token-based at the DOM root', () => {
    const root = document.createElement('div');

    applyTheme(root, DEFAULT_DAHN_THEME);

    expect(root.style.getPropertyValue('--dahn-color-surface')).toBe(
      DEFAULT_DAHN_THEME.colorTokens['surface'],
    );
    expect(root.style.getPropertyValue('--dahn-space-gap')).toBe(
      DEFAULT_DAHN_THEME.spacingTokens['gap'],
    );
  });
});
