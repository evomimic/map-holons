import { describe, expect, it, vi } from 'vitest';
import { MaterializedVisualizerCache, type VisualizerMaterializer } from './materialized-visualizer-cache';

describe('MaterializedVisualizerCache', () => {
  it('materializes once per selected Visualizer key', async () => {
    const materialize = vi.fn<VisualizerMaterializer['materialize']>().mockResolvedValue({
      source: 'export default class Visualizer {}', format: 'ESModule', entrypoint: 'default',
    });
    const cache = new MaterializedVisualizerCache({ materialize });
    const selected = { versionedKey: vi.fn().mockResolvedValue('PathInspector.RootedNavigationVisualizer') };

    await Promise.all([cache.get(selected as never), cache.get(selected as never)]);
    expect(materialize).toHaveBeenCalledTimes(1);
  });
});

it('keeps executable versions distinct even when their semantic base key is shared', async () => {
  const materialize = vi.fn(async () => ({ source: 'verified', format: 'ESModule' as const, entrypoint: 'default' }));
  const cache = new MaterializedVisualizerCache({ materialize });
  const old = { versionedKey: async () => 'Widget-v1' }, current = { versionedKey: async () => 'Widget-v2' };
  await cache.get(old as never); await cache.get(current as never); await cache.get(old as never);
  expect(materialize).toHaveBeenCalledTimes(2);
});
it('consumes supplied version metadata without fetching the selected definition again', async () => {
  const materialize = vi.fn(async () => ({ source: 'verified', format: 'ESModule' as const, entrypoint: 'default' }));
  const selected = { versionedKey: vi.fn(() => { throw new Error('unexpected definition read'); }) };
  const cache = new MaterializedVisualizerCache({ materialize }); cache.seedIdentity(selected as never,'Widget-v2');
  await cache.get(selected as never); expect(selected.versionedKey).not.toHaveBeenCalled(); expect(materialize).toHaveBeenCalledOnce();
});
