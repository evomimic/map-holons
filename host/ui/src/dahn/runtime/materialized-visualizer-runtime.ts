import type { HolonReference } from '../deps';
import type {
  MaterializedVisualizerCache,
  MaterializedVisualizerModule,
} from './materialized-visualizer-cache';

export type VisualizerModuleExports = Record<string, unknown>;

/** Imports one ES module source string supplied by MaterializeVisualizer. */
export type VisualizerModuleImporter = (
  source: string,
) => Promise<VisualizerModuleExports>;

/**
 * Realizes a Rust-selected Visualizer from its materialized module.
 *
 * The selected Visualizer is the sole cache key and authority for executable
 * code; this runtime does not maintain a second implementation registry.
 */
export class MaterializedVisualizerRuntime {
  private readonly importedModules = new Map<string, Promise<VisualizerModuleExports>>();

  constructor(
    private readonly cache: MaterializedVisualizerCache,
    private readonly importModule: VisualizerModuleImporter = importEsModule,
  ) {}

  async realize(selectedVisualizer: HolonReference): Promise<unknown> {
    const module = await this.cache.get(selectedVisualizer);
    // Identical materialized source has one constructor identity, including
    // concurrent requests for different semantic Visualizers sharing code.
    let imported = this.importedModules.get(module.source);
    if (imported === undefined) {
      imported = this.importModule(module.source);
      this.importedModules.set(module.source, imported);
      imported.catch(() => this.importedModules.delete(module.source));
    }
    const exports = await imported;
    const entrypoint = exports[module.entrypoint];

    if (entrypoint === undefined) {
      throw new Error(
        `Materialized Visualizer module does not export '${module.entrypoint}'`,
      );
    }

    return entrypoint;
  }
}

async function importEsModule(
  source: MaterializedVisualizerModule['source'],
): Promise<VisualizerModuleExports> {
  const sourceUrl = URL.createObjectURL(
    new Blob([source], { type: 'text/javascript' }),
  );

  try {
    return (await import(/* @vite-ignore */ sourceUrl)) as VisualizerModuleExports;
  } finally {
    URL.revokeObjectURL(sourceUrl);
  }
}
