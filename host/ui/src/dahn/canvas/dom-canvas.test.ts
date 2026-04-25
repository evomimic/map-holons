import { describe, expect, it, vi } from 'vitest';
import { DomCanvas } from './dom-canvas';
import { DefaultVisualizerRegistry } from '../registry/default-visualizer-registry';
import type { VisualizerContext, VisualizerElement } from '../contracts/visualizers';
import type { DahnTarget } from '../contracts/targets';
import { DEFAULT_DAHN_THEME } from '../themes/default-theme';

class TestCanvasVisualizerElement
  extends HTMLElement
  implements VisualizerElement
{
  context?: VisualizerContext;

  setContext(context: VisualizerContext): void {
    this.context = context;
    this.dataset['contextApplied'] = 'true';
  }
}

function createTestCanvasVisualizerElementClass(): typeof TestCanvasVisualizerElement {
  return class extends TestCanvasVisualizerElement {};
}

describe('DomCanvas', () => {
  it('applies theme tokens to the canvas root', () => {
    const container = document.createElement('div');
    const registry = new DefaultVisualizerRegistry();
    const canvas = new DomCanvas(
      container,
      registry,
      () => ({}) as VisualizerContext,
    );

    canvas.setTheme(DEFAULT_DAHN_THEME);

    const root = container.querySelector('[data-dahn-canvas="root"]');
    expect(root).not.toBeNull();
    expect(root?.style.getPropertyValue('--dahn-color-surface')).toBe(
      DEFAULT_DAHN_THEME.colorTokens['surface'],
    );
  });

  it('mounts a loaded visualizer and calls setContext', async () => {
    const container = document.createElement('div');
    const registry = new DefaultVisualizerRegistry();
    const target = { reference: {} } as DahnTarget;
    const context = {
      target,
      holon: {} as VisualizerContext['holon'],
      actions: [],
      theme: DEFAULT_DAHN_THEME,
      canvas: {} as VisualizerContext['canvas'],
    } as VisualizerContext;
    const resolveContext = vi.fn(() => context);

    registry.register({
      id: 'debug',
      displayName: 'Debug',
      version: '0.0.0',
      componentTag: 'test-canvas-visualizer',
      supportedTargets: [{ kind: 'debug' }],
      load: async () => {
        if (customElements.get('test-canvas-visualizer') === undefined) {
          customElements.define(
            'test-canvas-visualizer',
            createTestCanvasVisualizerElementClass(),
          );
        }
      },
    });

    const canvas = new DomCanvas(container, registry, resolveContext);
    await canvas.mountVisualizers([
      {
        visualizerId: 'debug',
        target,
        slot: 'primary',
      },
    ]);

    const mounted = container.querySelector(
      'test-canvas-visualizer',
    ) as TestCanvasVisualizerElement | null;

    expect(resolveContext).toHaveBeenCalledWith(target);
    expect(mounted).not.toBeNull();
    expect(mounted?.dataset['contextApplied']).toBe('true');
    expect(mounted?.context).toBe(context);
  });

  it('clears the primary slot', async () => {
    const container = document.createElement('div');
    const registry = new DefaultVisualizerRegistry();
    const target = { reference: {} } as DahnTarget;

    registry.register({
      id: 'debug',
      displayName: 'Debug',
      version: '0.0.0',
      componentTag: 'test-canvas-visualizer-clear',
      supportedTargets: [{ kind: 'debug' }],
      load: async () => {
        if (
          customElements.get('test-canvas-visualizer-clear') === undefined
        ) {
          customElements.define(
            'test-canvas-visualizer-clear',
            createTestCanvasVisualizerElementClass(),
          );
        }
      },
    });

    const canvas = new DomCanvas(
      container,
      registry,
      () => ({}) as VisualizerContext,
    );

    await canvas.mountVisualizers([
      { visualizerId: 'debug', target, slot: 'primary' },
    ]);
    expect(container.querySelector('test-canvas-visualizer-clear')).not.toBeNull();

    canvas.clear();

    expect(container.querySelector('test-canvas-visualizer-clear')).toBeNull();
  });
});
