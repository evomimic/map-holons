import { describe, expect, it, vi } from 'vitest';
import { DefaultVisualizerRegistry } from './default-visualizer-registry';

describe('DefaultVisualizerRegistry', () => {
  it('registers and lists definitions', () => {
    const registry = new DefaultVisualizerRegistry();
    const definition = {
      id: 'debug',
      displayName: 'Debug',
      version: '0.0.0',
      componentTag: 'test-debug-visualizer-a',
      supportedTargets: [{ kind: 'debug' as const }],
      load: vi.fn(async () => {}),
    };

    registry.register(definition);

    expect(registry.get('debug')).toBe(definition);
    expect(registry.list()).toEqual([definition]);
  });

  it('rejects duplicate ids for different definitions', () => {
    const registry = new DefaultVisualizerRegistry();
    registry.register({
      id: 'debug',
      displayName: 'Debug',
      version: '0.0.0',
      componentTag: 'test-debug-visualizer-b',
      supportedTargets: [{ kind: 'debug' as const }],
      load: async () => {},
    });

    expect(() =>
      registry.register({
        id: 'debug',
        displayName: 'Debug 2',
        version: '0.0.1',
        componentTag: 'test-debug-visualizer-c',
        supportedTargets: [{ kind: 'debug' as const }],
        load: async () => {},
      }),
    ).toThrow(/already registered/);
  });

  it('indexes executable definitions by implementation key without replacing their local ids', async () => {
    const registry = new DefaultVisualizerRegistry();
    const definition = {
      id: 'local-node-definition',
      implementationKey: 'dahn.generic-holon-node',
      displayName: 'Holon Node',
      version: '0.0.0',
      componentTag: 'test-implementation-key-visualizer',
      supportedTargets: [{ kind: 'holon-node' as const }],
      load: async () => {
        class TestVisualizer extends HTMLElement {}
        if (
          customElements.get('test-implementation-key-visualizer') ===
          undefined
        ) {
          customElements.define(
            'test-implementation-key-visualizer',
            TestVisualizer,
          );
        }
      },
    };

    registry.register(definition);

    expect(registry.get('local-node-definition')).toBe(definition);
    expect(registry.getByImplementationKey('dahn.generic-holon-node')).toBe(
      definition,
    );
    await expect(
      registry.ensureImplementationLoaded('dahn.generic-holon-node'),
    ).resolves.toBe(definition);
  });

  it('rejects duplicate implementation keys for different definitions', () => {
    const registry = new DefaultVisualizerRegistry();
    registry.register({
      id: 'first-definition',
      implementationKey: 'dahn.space-navigator',
      displayName: 'First',
      version: '0.0.0',
      componentTag: 'test-first-implementation-key-visualizer',
      supportedTargets: [{ kind: 'canvas' }],
      load: async () => {},
    });

    expect(() =>
      registry.register({
        id: 'second-definition',
        implementationKey: 'dahn.space-navigator',
        displayName: 'Second',
        version: '0.0.0',
        componentTag: 'test-second-implementation-key-visualizer',
        supportedTargets: [{ kind: 'canvas' }],
        load: async () => {},
      }),
    ).toThrow(/implementation key.*already registered/);
    expect(registry.get('second-definition')).toBeUndefined();
  });

  it('loads definitions idempotently', async () => {
    const registry = new DefaultVisualizerRegistry();
    const load = vi.fn(async () => {
      class TestVisualizer extends HTMLElement {}
      if (customElements.get('test-debug-visualizer-d') === undefined) {
        customElements.define('test-debug-visualizer-d', TestVisualizer);
      }
    });

    registry.register({
      id: 'debug',
      displayName: 'Debug',
      version: '0.0.0',
      componentTag: 'test-debug-visualizer-d',
      supportedTargets: [{ kind: 'debug' as const }],
      load,
    });

    await registry.ensureLoaded('debug');
    await registry.ensureLoaded('debug');

    expect(load).toHaveBeenCalledTimes(1);
    expect(customElements.get('test-debug-visualizer-d')).toBeDefined();
  });

  it('does not re-run load when the custom element is already defined', async () => {
    class ExistingVisualizer extends HTMLElement {}
    if (customElements.get('test-debug-visualizer-e') === undefined) {
      customElements.define('test-debug-visualizer-e', ExistingVisualizer);
    }

    const registry = new DefaultVisualizerRegistry();
    const load = vi.fn(async () => {});

    registry.register({
      id: 'debug',
      displayName: 'Debug',
      version: '0.0.0',
      componentTag: 'test-debug-visualizer-e',
      supportedTargets: [{ kind: 'debug' as const }],
      load,
    });

    await registry.ensureLoaded('debug');

    expect(load).not.toHaveBeenCalled();
  });
});
