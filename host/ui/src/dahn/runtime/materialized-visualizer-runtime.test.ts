import { describe, expect, it, vi } from 'vitest';
import {
  MaterializedVisualizerCache,
  type VisualizerMaterializer,
} from './materialized-visualizer-cache';
import { MaterializedVisualizerRuntime } from './materialized-visualizer-runtime';

describe('MaterializedVisualizerRuntime', () => {
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
