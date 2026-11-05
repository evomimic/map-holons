import { describe, expect, it, vi } from 'vitest';
import {
  MaterializedVisualizerCache,
  type VisualizerMaterializer,
} from './materialized-visualizer-cache';
import { MaterializedVisualizerRuntime } from './materialized-visualizer-runtime';

describe('MaterializedVisualizerRuntime', () => {
  it('shares constructor identity while materialization belongs to separate disposable presentation contexts', async () => {
    const artifact = { source: 'verified explorer', format: 'ESModule' as const, entrypoint: 'default' };
    const base = vi.fn().mockResolvedValue(artifact), first = vi.fn().mockResolvedValue(artifact), second = vi.fn().mockResolvedValue(artifact);
    const implementation = class Explorer {};
    const importer = vi.fn().mockResolvedValue({ default: implementation });
    const runtime = new MaterializedVisualizerRuntime(new MaterializedVisualizerCache({ materialize: base }), importer);
    const selected = { key: async () => 'explorer' };
    await expect(runtime.realize(selected as never, new MaterializedVisualizerCache({ materialize: first }))).resolves.toBe(implementation);
    await expect(runtime.realize(selected as never, new MaterializedVisualizerCache({ materialize: second }))).resolves.toBe(implementation);
    expect(first).toHaveBeenCalledOnce(); expect(second).toHaveBeenCalledOnce();
    expect(base).not.toHaveBeenCalled(); expect(importer).toHaveBeenCalledOnce();
  });
  it('realizes the entrypoint from the selected Visualizer materialization', async () => {
    const materialize = vi
      .fn<VisualizerMaterializer['materialize']>()
      .mockResolvedValue({
        source: 'export default class Visualizer {}',
        format: 'ESModule',
        entrypoint: 'default',
      });
    const visualizer = { key: vi.fn().mockResolvedValue('selected.visualizer') };
    const implementation = class Visualizer {};
    const importModule = vi.fn().mockResolvedValue({ default: implementation });
    const runtime = new MaterializedVisualizerRuntime(
      new MaterializedVisualizerCache({ materialize }),
      importModule,
    );

    await expect(runtime.realize(visualizer as never)).resolves.toBe(
      implementation,
    );
    expect(materialize).toHaveBeenCalledWith(visualizer);
    expect(importModule).toHaveBeenCalledWith(
      'export default class Visualizer {}',
    );
  });

  it('shares constructor identity for concurrent requests using the same materialized source', async () => {
    const materialize = vi.fn().mockResolvedValue({ source: 'shared source', format: 'ESModule', entrypoint: 'default' });
    const constructor = class Selected {};
    const importer = vi.fn().mockResolvedValue({ default: constructor });
    const runtime = new MaterializedVisualizerRuntime(new MaterializedVisualizerCache({ materialize }), importer);
    const first = { key: async () => 'first' };
    const second = { key: async () => 'second' };
    expect(await Promise.all([runtime.realize(first as never), runtime.realize(second as never)])).toEqual([constructor, constructor]);
    expect(importer).toHaveBeenCalledTimes(1);
  });

  it('surfaces realization failure and permits retry without a fallback', async () => {
    const cache = new MaterializedVisualizerCache({ materialize: vi.fn().mockResolvedValue({ source: 'selected', format: 'ESModule', entrypoint: 'default' }) });
    const constructor = class Selected {};
    const importer = vi.fn().mockRejectedValueOnce(new Error('unavailable selected module')).mockResolvedValue({ default: constructor });
    const runtime = new MaterializedVisualizerRuntime(cache, importer);
    const selected = { key: async () => 'selected' };
    await expect(runtime.realize(selected as never)).rejects.toThrow('unavailable selected module');
    await expect(runtime.realize(selected as never)).resolves.toBe(constructor);
  });

  it('rejects a module that omits its declared entrypoint', async () => {
    const cache = new MaterializedVisualizerCache({
      materialize: vi.fn().mockResolvedValue({
        source: 'export const named = true',
        format: 'ESModule',
        entrypoint: 'default',
      }),
    });
    const runtime = new MaterializedVisualizerRuntime(cache, async () => ({}));
    const visualizer = { key: vi.fn().mockResolvedValue('selected.visualizer') };

    await expect(runtime.realize(visualizer as never)).rejects.toThrow(
      "does not export 'default'",
    );
  });
});

it('resolves an inherited type-owned Slot without importing the Visualizer module', async () => {
  const ref = (key: string, role?: string, edges: Record<string, any[]> = {}): any => ({
    identity: key, equals: (other: any) => other.identity === key,
    propertyValue: async () => role === undefined ? null : { StringValue: role },
    relatedHolons: async (name: string) => edges[name] ?? [],
  });
  const inherited = ref('inherited', 'details');
  const overridden = ref('overridden', 'details');
  const base = ref('base', undefined, { HasSlot: [inherited] });
  const concrete = ref('concrete', undefined, { Extends: [base] });
  const parent = ref('parent', undefined, { DescribedBy: [concrete] });
  const materialize = vi.fn();
  const runtime = new MaterializedVisualizerRuntime(new MaterializedVisualizerCache({ materialize }));
  await expect(runtime.slot(parent, 'details')).resolves.toBe(inherited);
  concrete.relatedHolons = async (name: string) => ({ HasSlot: [overridden], Extends: [base] })[name] ?? [];
  await expect(runtime.slot(parent, 'details')).resolves.toBe(overridden);
  expect(materialize).not.toHaveBeenCalled();
});

it('rejects ambiguous type-owned roles instead of using an ancestor Slot', async () => {
  const slot = () => ({ propertyValue: async () => ({ StringValue: 'details' }) });
  const descriptor = { relatedHolons: async () => [slot(), slot()] };
  const parent = { relatedHolons: async () => [descriptor] };
  const runtime = new MaterializedVisualizerRuntime(null as never);
  await expect(runtime.slot(parent as never, 'details')).rejects.toThrow('Duplicate composition role');
});
