import type { MapTransaction, HolonReference } from '../deps';
import type {
  MaterializedVisualizerModule,
  VisualizerMaterializer,
} from '../runtime/materialized-visualizer-cache';

/**
 * DAHN adapter for the public SDK materialization boundary.
 *
 * It converts a Rust-authorized, one-use artifact capability into the
 * in-memory module source required by the UI runtime. It has no selection,
 * implementation, or fallback policy.
 */
export class SdkVisualizerMaterializer implements VisualizerMaterializer {
  constructor(private readonly transaction: MapTransaction) {}

  async materialize(
    selectedVisualizer: HolonReference,
  ): Promise<MaterializedVisualizerModule> {
    const materialized = await this.transaction.materializeVisualizer(selectedVisualizer);
    if (materialized.format !== 'ESModule') {
      throw new Error(
        `Unsupported MaterializedVisualizer module format '${materialized.format}'`,
      );
    }

    const bytes = await this.transaction.fetchArtifact(materialized.artifactHandle);
    let source: string;
    try {
      source = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
    } catch {
      throw new Error('MaterializedVisualizer artifact is not valid UTF-8 JavaScript source');
    }

    return { source, format: 'ESModule', entrypoint: materialized.entrypoint };
  }
}
