import { describe, expect, it, vi } from 'vitest';
import { MaterializedVisualizerCache, type VisualizerMaterializer } from './materialized-visualizer-cache';

describe('MaterializedVisualizerCache', () => {
  it('materializes once per selected Visualizer key', async () => {
    const materialize = vi.fn<VisualizerMaterializer['materialize']>().mockResolvedValue({
      source: 'export default class Visualizer {}', format: 'ESModule', entrypoint: 'default',
    });
    const cache = new MaterializedVisualizerCache({ materialize });
    const selected = { key: vi.fn().mockResolvedValue('GenericHolonNodeVisualizer.NodeVisualizer') };

    await Promise.all([cache.get(selected as never), cache.get(selected as never)]);
    expect(materialize).toHaveBeenCalledTimes(1);
  });
});
