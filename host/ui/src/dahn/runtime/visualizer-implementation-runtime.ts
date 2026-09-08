import type {
  VisualizerDefinition,
  VisualizerImplementationResolution,
} from '../contracts/visualizers';
import type { VisualizerRegistry } from '../registry/visualizer-registry';
import { DahnImplementationResolutionError } from './runtime-errors';

/** Resolves one Rust-selected implementation to a locally executable definition. */
export interface VisualizerImplementationRuntime {
  resolve(
    resolution: VisualizerImplementationResolution,
  ): Promise<VisualizerDefinition>;
}

export class DefaultVisualizerImplementationRuntime
  implements VisualizerImplementationRuntime
{
  constructor(private readonly registry: VisualizerRegistry) {}

  async resolve(
    resolution: VisualizerImplementationResolution,
  ): Promise<VisualizerDefinition> {
    if (resolution.runtime !== 'TypeScript') {
      throw new DahnImplementationResolutionError(
        'unsupported-runtime',
        resolution,
      );
    }

    if (
      resolution.implementationKey === undefined ||
      resolution.implementationKey.trim() === ''
    ) {
      throw new DahnImplementationResolutionError(
        'missing-implementation-key',
        resolution,
      );
    }

    try {
      return await this.registry.ensureImplementationLoaded(
        resolution.implementationKey,
      );
    } catch (error) {
      const reason =
        this.registry.getByImplementationKey(resolution.implementationKey) ===
        undefined
          ? 'unknown-implementation-key'
          : 'load-failed';
      throw new DahnImplementationResolutionError(reason, resolution, {
        cause: error,
      });
    }
  }
}
