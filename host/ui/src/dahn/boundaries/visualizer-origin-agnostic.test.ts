import { describe, expect, it } from 'vitest';
import { DefaultVisualizerRegistry } from '../registry/default-visualizer-registry';
import { Phase0Selector } from '../selector/phase0-selector';
import type {
  CanvasDescriptor,
  DahnTarget,
  HolonViewAccess,
  SelectorInput,
  VisualizerDefinition,
} from '../index';

const canvas: CanvasDescriptor = {
  id: 'dahn-origin-agnostic',
  slots: ['primary'],
};

function makeDefinition(
  id: string,
  componentTag: string,
  extraMetadata?: Record<string, unknown>,
): VisualizerDefinition {
  return {
    id,
    displayName: id,
    version: '0.0.0',
    componentTag,
    supportedTargets: [
      { kind: (id === 'action-menu' ? 'action' : 'holon-node') as const },
    ],
    load: async () => {},
    ...(extraMetadata as object),
  };
}

describe('DAHN visualizer origin-agnostic seams', () => {
  it('keys registry behavior by visualizer id rather than source metadata', () => {
    const registry = new DefaultVisualizerRegistry();
    const definition = makeDefinition(
      'holon-node',
      'visualizer-from-trust-channel',
      {
        sourceUrl: 'https://example.invalid/plugin.js',
        trustChannelId: 'trust-channel-1',
      },
    );

    registry.register(definition);

    expect(registry.get('holon-node')).toBe(definition);
  });

  it('keeps selector output independent from component tag and origin metadata', () => {
    const selector = new Phase0Selector();
    const target = { reference: {} } as DahnTarget;
    const input: SelectorInput = {
      target,
      holon: {} as HolonViewAccess,
      actions: [{ id: 'open', kind: 'action', label: 'Open' }],
      availableVisualizers: [
        makeDefinition('holon-node', 'visualizer-from-i-space', {
          signedBundleId: 'bundle-a',
        }),
        makeDefinition('action-menu', 'visualizer-from-integration-hub', {
          trustChannelId: 'trust-channel-2',
        }),
      ],
      canvas,
    };

    expect(selector.select(input)).toEqual({
      visualizers: [
        { visualizerId: 'holon-node', target, slot: 'primary' },
        { visualizerId: 'action-menu', target, slot: 'primary' },
      ],
    });
  });

  it('treats load as an activation seam rather than a URL contract', async () => {
    const registry = new DefaultVisualizerRegistry();
    let activated = false;

    registry.register({
      id: 'debug',
      displayName: 'Debug',
      version: '0.0.0',
      componentTag: 'origin-agnostic-debug-visualizer',
      supportedTargets: [{ kind: 'debug' }],
      load: async () => {
        activated = true;
        class OriginAgnosticDebugVisualizer extends HTMLElement {}
        if (
          customElements.get('origin-agnostic-debug-visualizer') === undefined
        ) {
          customElements.define(
            'origin-agnostic-debug-visualizer',
            OriginAgnosticDebugVisualizer,
          );
        }
      },
    });

    await registry.ensureLoaded('debug');

    expect(activated).toBe(true);
    expect(customElements.get('origin-agnostic-debug-visualizer')).toBeDefined();
  });
});
