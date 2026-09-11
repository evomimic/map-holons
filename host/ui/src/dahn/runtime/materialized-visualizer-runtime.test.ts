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
