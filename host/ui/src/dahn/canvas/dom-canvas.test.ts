import { describe, expect, it, vi } from 'vitest';
import { DomCanvas } from './dom-canvas';
import { DefaultVisualizerRegistry } from '../registry/default-visualizer-registry';
import type { VisualizerContext, VisualizerElement } from '../contracts/visualizers';
import type { DahnTarget } from '../contracts/targets';
import type { DahnTheme } from '../contracts/themes';
import type { Theme } from '../themes/theme';

const THEME: DahnTheme = {
  themeKey: 'DAHN.DefaultTheme',
  themeVersionedKey: 'DAHN.DefaultTheme@1',
  metaDesignSystemKey: 'DAHN.DefaultMetaDesignSystem',
  metaDesignSystemVersionedKey: 'DAHN.DefaultMetaDesignSystem@1',
  cssCustomProperties: {
    '--dahn-canvas-surface-background': '#f7f5ef',
  },
};

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
  it('separates Canvas chrome from the hosted-Dancer allocation', () => {
    const container = document.createElement('div');
    new DomCanvas(container, new DefaultVisualizerRegistry(), () => ({}) as VisualizerContext);

    expect(container.querySelector('[data-dahn-canvas-chrome="true"]')).not.toBeNull();
    expect(container.querySelector('[data-dahn-hosted-dancer-region="true"]')).not.toBeNull();
    expect(container.querySelector('[data-dahn-canvas-empty-state="true"]')?.textContent).toBe(
      'Awaiting home Dancer',
    );
  });

  it('fills its allocated container and gives the hosted experience the remaining height', () => {
    const container = document.createElement('div');
    new DomCanvas(container, new DefaultVisualizerRegistry(), () => ({}) as VisualizerContext);

    const root = container.querySelector<HTMLElement>('[data-dahn-canvas="root"]');
    const hostedDancerRegion = container.querySelector<HTMLElement>(
      '[data-dahn-hosted-dancer-region="true"]',
    );
    const primarySlot = container.querySelector<HTMLElement>('[data-dahn-canvas-slot="primary"]');

    expect(root?.style.height).toBe('100%');
    expect(hostedDancerRegion?.style.flexGrow).toBe('1');
    expect(primarySlot?.style.flexGrow).toBe('1');
  });

  it('resolves and applies Theme once during asynchronous canvas initialization', async () => {
    const container = document.createElement('div');
    const registry = new DefaultVisualizerRegistry();
    const toCssCustomProperties = vi.fn(async () => THEME);
    const resolveTheme = vi.fn(async () => ({
      toCssCustomProperties,
    }) as unknown as Theme);

    const canvas = await DomCanvas.create(
      container,
      registry,
      () => ({}) as VisualizerContext,
      { resolveTheme },
    );

    await canvas.mountVisualizers([]);

    expect(resolveTheme).toHaveBeenCalledTimes(1);
    expect(toCssCustomProperties).toHaveBeenCalledTimes(1);
  });

  it('applies theme tokens to the canvas root', () => {
    const container = document.createElement('div');
    const registry = new DefaultVisualizerRegistry();
    const canvas = new DomCanvas(
      container,
      registry,
      () => ({}) as VisualizerContext,
    );

    canvas.setTheme(THEME);

    const root = container.querySelector('[data-dahn-canvas="root"]');
    expect(root).not.toBeNull();
    expect(root?.style.getPropertyValue('--dahn-canvas-surface-background')).toBe(
      THEME.cssCustomProperties['--dahn-canvas-surface-background'],
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
      theme: THEME,
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
    expect(
      container.querySelector<HTMLElement>('[data-dahn-canvas-empty-state="true"]')?.hidden,
    ).toBe(true);
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
    expect(
      container.querySelector('[data-dahn-hosted-dancer-region="true"]')?.getAttribute(
        'data-dahn-canvas-state',
      ),
    ).toBe('awaiting-home-dancer');
  });
});

it('keeps Canvas chrome and successful mounts when another visualizer fails', async () => {
  const diagnostic = vi.spyOn(console, 'error').mockImplementation(() => {});
  const container = document.createElement('div');
  const registry = new DefaultVisualizerRegistry();
  customElements.define('test-healthy-sibling', createTestCanvasVisualizerElementClass());
  registry.register({
    id: 'healthy', displayName: 'Healthy', version: '1',
    componentTag: 'test-healthy-sibling', supportedTargets: [{ kind: 'holon-node' }],
    load: async () => {},
  });
  const canvas = new DomCanvas(container, registry, () => ({}) as VisualizerContext);
  const target = {} as DahnTarget;
  await canvas.mountVisualizers([
    { visualizerId: 'missing', target, slot: 'primary' },
    { visualizerId: 'healthy', target, slot: 'primary' },
  ]);
  expect(container.querySelector('[data-dahn-canvas-chrome]')).not.toBeNull();
  expect(container.querySelector('[data-dahn-region-state="unavailable"]')).not.toBeNull();
  expect(container.querySelector('test-healthy-sibling')?.getAttribute('data-context-applied')).toBe('true');
  diagnostic.mockRestore();
});
