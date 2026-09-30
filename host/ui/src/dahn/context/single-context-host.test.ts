import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SingleContextHost } from './single-context-host';
import { DomCanvas } from '../canvas/dom-canvas';
import { DefaultVisualizerRegistry } from '../registry/default-visualizer-registry';
import type { ContextId, ContextPresentation, ContextRequestResult, RetainedContext } from '../contracts/context-host';
import type { VisualizerContext } from '../contracts/visualizers';

class DisplayObserver {
  static instances: DisplayObserver[] = [];
  target!: Element;
  disconnect = vi.fn();
  constructor(private readonly callback: ResizeObserverCallback) { DisplayObserver.instances.push(this); }
  observe(target: Element) { this.target = target; }
  resize(width: number, height: number) {
    this.callback([{ target: this.target, contentRect: { width, height } } as ResizeObserverEntry], this as unknown as ResizeObserver);
  }
}

function applied(result: ContextRequestResult<RetainedContext>): RetainedContext {
  expect(result.status).toBe('applied');
  if (result.status !== 'applied') throw new Error('Expected context creation.');
  return result.value;
}

beforeEach(() => { DisplayObserver.instances = []; vi.stubGlobal('ResizeObserver', DisplayObserver); });
afterEach(() => { vi.unstubAllGlobals(); document.body.replaceChildren(); });

describe('SingleContextHost', () => {
  it('hosts a generic Canvas with stable identity, bounded updates and a separately owned semantic lifetime', async () => {
    const semanticOwner = { staged: ['retained edit'], commit: vi.fn(), abandon: vi.fn(), revert: vi.fn() };
    const display = document.createElement('section');
    display.style.width = '800px';
    document.body.append(display);
    const host = new SingleContextHost(display);
    let canvas!: DomCanvas;
    const allocations = vi.fn();
    const dispose = vi.fn(() => canvas.dispose());
    const context = applied(host.create((container, parent) => {
      canvas = new DomCanvas(container, new DefaultVisualizerRegistry(), () => ({}) as VisualizerContext, parent);
      return { setAllocation: bounds => { allocations(bounds); canvas.setAllocation(bounds); }, dispose, ready: canvas.mountVisualizers([]) };
    }));
    const observer = DisplayObserver.instances[0];
    expect(context.allocation).toBeNull();
    observer.resize(400, 300);
    expect((await context.ready).status).toBe('applied');
    expect(canvas.context?.id).toBe(context.id);
    expect(canvas.rootElement().style.width).toBe('400px');
    expect(canvas.rootElement().style.height).toBe('300px');
    expect(context.request('activate')).toEqual({ status: 'already-satisfied' });
    expect(host.capabilities).toEqual({ create: true, createAdditional: false, activate: true, destroy: true, maximize: false, restore: false, minimize: false });
    expect(Object.isFrozen(context.allocation)).toBe(true);
    const identity = context.id;
    observer.resize(500, 350);
    expect(context.id).toBe(identity);
    expect(context.allocation).toEqual({ width: 500, height: 350 });
    expect(allocations).toHaveBeenCalledTimes(2);
    expect(canvas.rootElement().style.width).toBe('500px');
    expect(display.style.width).toBe('800px');
    for (const operation of ['maximize', 'restore', 'minimize'] as const) {
      expect(canvas.context?.request(operation).status).toBe('unsupported');
    }
    const secondMount = vi.fn();
    expect(host.create(secondMount).status).toBe('unsupported');
    expect(secondMount).not.toHaveBeenCalled();
    expect(host.request('unknown' as ContextId, 'destroy').status).toBe('refused');
    expect(context.allocation).toEqual({ width: 500, height: 350 });
    expect(canvas.rootElement().isConnected).toBe(true);
    expect(context.request('destroy').status).toBe('applied');
    expect(dispose).toHaveBeenCalledTimes(1);
    expect(observer.disconnect).toHaveBeenCalledTimes(1);
    expect(display.children).toHaveLength(0);
    expect(semanticOwner.staged).toEqual(['retained edit']);
    expect(semanticOwner.commit).not.toHaveBeenCalled();
    expect(semanticOwner.abandon).not.toHaveBeenCalled();
    expect(semanticOwner.revert).not.toHaveBeenCalled();
    expect(context.request('activate').status).toBe('refused');
    host.dispose();
    expect(dispose).toHaveBeenCalledTimes(1);
  });

  it('waits for usable bounds and retains the last valid grant across hidden or invalid measurements', async () => {
    const presentation = { setAllocation: vi.fn(), dispose: vi.fn() };
    const mount = vi.fn(() => presentation);
    const host = new SingleContextHost(document.createElement('div'));
    const context = applied(host.create(mount));
    const observer = DisplayObserver.instances[0];
    for (const [width, height] of [[0, 10], [10, 0], [-1, 10], [Infinity, 10], [10, NaN]]) observer.resize(width, height);
    expect(mount).not.toHaveBeenCalled();
    observer.resize(320.5, 200.5);
    await context.ready;
    expect(mount).toHaveBeenCalledTimes(1);
    observer.resize(0, 0);
    observer.resize(Infinity, 10);
    observer.resize(320.5, 200.5);
    expect(context.allocation).toEqual({ width: 320.5, height: 200.5 });
    expect(presentation.setAllocation).toHaveBeenCalledTimes(1);
    host.dispose();
  });

  it('destroys a context awaiting bounds, then creates a new identity without accepting stale notifications', async () => {
    const host = new SingleContextHost(document.createElement('div'));
    const mount = vi.fn(() => ({ setAllocation: vi.fn(), dispose: vi.fn() }));
    const first = applied(host.create(mount));
    first.request('destroy');
    expect((await first.ready).status).toBe('refused');
    const second = applied(host.create(mount));
    expect(second.id).not.toBe(first.id);
    DisplayObserver.instances[0].resize(500, 500);
    expect(mount).not.toHaveBeenCalled();
    DisplayObserver.instances[1].resize(300, 200);
    expect((await second.ready).status).toBe('applied');
    host.dispose();
    host.dispose();
    expect(host.create(mount).status).toBe('refused');
  });

  it('releases presentation immediately during asynchronous realization and consumes its late rejection', async () => {
    let reject!: (error: Error) => void;
    let signal!: AbortSignal;
    const presentation: ContextPresentation = {
      setAllocation: vi.fn(), dispose: vi.fn(),
      ready: new Promise((_resolve, fail) => { reject = fail; }),
    };
    const display = document.createElement('div');
    const host = new SingleContextHost(display);
    const context = applied(host.create((_container, _context, cancellation) => { signal = cancellation; return presentation; }));
    DisplayObserver.instances[0].resize(300, 200);
    host.dispose();
    expect(signal.aborted).toBe(true);
    expect(presentation.dispose).toHaveBeenCalledTimes(1);
    expect((await context.ready).status).toBe('refused');
    reject(new Error('late mount failure'));
    await Promise.resolve();
    expect(display.children).toHaveLength(0);
  });

  it('reports mount failure, cleans the allocation and allows a fresh context', async () => {
    const display = document.createElement('div');
    const host = new SingleContextHost(display);
    const error = new Error('realization failed');
    const dispose = vi.fn();
    const context = applied(host.create(() => ({ setAllocation: vi.fn(), dispose, ready: Promise.reject(error) })));
    DisplayObserver.instances[0].resize(300, 200);
    expect(await context.ready).toMatchObject({ status: 'refused', error });
    expect(dispose).toHaveBeenCalledTimes(1);
    expect(display.children).toHaveLength(0);
    expect(host.create(() => ({ setAllocation: vi.fn(), dispose: vi.fn() })).status).toBe('applied');
    host.dispose();
  });
});

it('consumes late failure and disposes resources returned by an adapter that destroys its own context', async () => {
  const host = new SingleContextHost(document.createElement('div'));
  const dispose = vi.fn();
  const context = applied(host.create((_container, parent) => {
    parent.request('destroy');
    return { dispose, setAllocation: vi.fn(), ready: Promise.reject(new Error('cancelled realization')) };
  }));
  DisplayObserver.instances[0].resize(300, 200);
  expect((await context.ready).status).toBe('refused');
  expect(dispose).toHaveBeenCalledTimes(1);
  host.dispose();
});
