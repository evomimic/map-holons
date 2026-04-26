import { describe, expect, it } from 'vitest';
import { DefaultVisualizerRegistry } from '../registry/default-visualizer-registry';
import { Phase0Selector } from '../selector/phase0-selector';
import { applyTheme } from '../themes/apply-theme';
import { DEFAULT_DAHN_THEME } from '../themes/default-theme';
import type {
  CanvasDescriptor,
  DahnTarget,
  HolonViewAccess,
  SelectorInput,
  VisualizerDefinition,
} from '../index';

const canvas: CanvasDescriptor = {
  id: 'dahn-structural',
  slots: ['primary'],
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
  it('keeps the selector as a deterministic TS-side presentation resolver', () => {
    const selector = new Phase0Selector();
    const input: SelectorInput = {
      target: { reference: {} } as DahnTarget,
      holon: {} as HolonViewAccess,
      actions: [],
      availableVisualizers: [visualizer('holon-node'), visualizer('action-menu')],
      canvas,
    };

    expect(selector.select(input)).toEqual(selector.select(input));
  });

  it('keeps the registry as an id-keyed loader registry rather than canvas logic', () => {
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
