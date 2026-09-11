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
