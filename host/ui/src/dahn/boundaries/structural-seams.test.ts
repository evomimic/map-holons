import { describe, expect, it } from 'vitest';
import { DefaultVisualizerRegistry } from '../registry/default-visualizer-registry';
import { applyTheme } from '../themes/apply-theme';
import type {
  DahnTheme,
  VisualizerDefinition,
} from '../index';

const THEME: DahnTheme = {
  themeKey: 'SpaceNavigator.DefaultTheme',
  themeVersionedKey: 'SpaceNavigator.DefaultTheme@1',
  metaDesignSystemKey: 'SpaceNavigator.MetaDesignSystem',
  metaDesignSystemVersionedKey: 'SpaceNavigator.MetaDesignSystem@1',
  cssCustomProperties: {
    '--dahn-canvas-surface-background': '#f7f5ef',
    '--dahn-canvas-gap': '16px',
  },
};

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

    applyTheme(root, THEME);

    expect(root.style.getPropertyValue('--dahn-canvas-surface-background')).toBe(
      THEME.cssCustomProperties['--dahn-canvas-surface-background'],
    );
    expect(root.style.getPropertyValue('--dahn-canvas-gap')).toBe(
      THEME.cssCustomProperties['--dahn-canvas-gap'],
    );
  });
});
