import type { HolonReference } from '../deps';

/** Rust-produced executable module projection after the Materialize dance. */
export interface MaterializedVisualizerModule {
  source: string;
  format: 'ESModule';
  entrypoint: string;
}

/**
 * Requests a materialization through the MAP adapter. The adapter owns the
 * DanceInvocation construction; the UI cache never reads an artifact itself.
 */
export interface VisualizerMaterializer {
  materialize(selectedVisualizer: HolonReference): Promise<MaterializedVisualizerModule>;
}

/**
 * Process-local cache keyed by a selected Visualizer's exact semantic version.
 *
 * It deliberately has no selector policy: cache misses return to
 * Rust through `VisualizerMaterializer`.
 */
export class MaterializedVisualizerCache {
  private readonly modules = new Map<string, Promise<MaterializedVisualizerModule>>();

  private readonly identities = new WeakMap<HolonReference, string>();
  seedIdentity(reference: HolonReference, versionIdentity: string): void { this.identities.set(reference, versionIdentity); }

  constructor(private readonly materializer: VisualizerMaterializer) {}

  async get(selectedVisualizer: HolonReference): Promise<MaterializedVisualizerModule> {
    const active = performance.getEntriesByName('map.navigation.active', 'mark').at(-1) as PerformanceMark | undefined;
    const started = performance.now();
    const key = this.identities.get(selectedVisualizer) ?? await selectedVisualizer.versionedKey();
    if (key === null) {
      throw new Error('A materialized Visualizer must have a stable semantic key');
    }
    let pending = this.modules.get(key);
    const hit = pending !== undefined;
    if (pending === undefined) {
      pending = this.materializer.materialize(selectedVisualizer);
      this.modules.set(key, pending);
      pending.catch(() => this.modules.delete(key));
    }
    try { return await pending; } finally {
      if (active) performance.measure('map.navigation.module', { start: started, end: performance.now(),
        detail: { run: active.detail, key, hit } });
    }
  }
}
