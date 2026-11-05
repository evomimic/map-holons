import { extractString } from '../deps/map-sdk';
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

  /** Resolve a semantic role from the selected Visualizer's TDL type definition. */
  async slot(parent: HolonReference, role: string): Promise<HolonReference> {
    const descriptors = await parent.relatedHolons('DescribedBy');
    if (descriptors.length !== 1) throw new Error('Expected one Visualizer type');
    let descriptor: HolonReference | undefined = [...descriptors][0];
    const visited: HolonReference[] = [];
    while (descriptor !== undefined) {
      if (visited.some(item => item.equals(descriptor!))) throw new Error('Cycle in Visualizer type ancestry');
      visited.push(descriptor);
      const matches: HolonReference[] = [];
      for (const slot of await descriptor.relatedHolons('HasSlot')) {
        const value = await slot.propertyValue('CompositionRole');
        if (value !== null && extractString(value) === role) matches.push(slot);
      }
      if (matches.length > 1) throw new Error(`Duplicate composition role ${role}`);
      if (matches.length === 1) return matches[0];
      const parents = await descriptor.relatedHolons('Extends');
      if (parents.length > 1) throw new Error('Multiple Visualizer type parents');
      descriptor = [...parents][0];
    }
    throw new Error(`Selected Visualizer type does not declare composition slot ${role}`);
  }

  /** Admission checks actual callable bindings for the slot's complete contract. */
  async admit(element: HTMLElement, slot: HolonReference): Promise<void> {
    const operations = (element as HTMLElement & { visualizerOperations?: Record<string, unknown> }).visualizerOperations;
    for (const operator of await slot.relatedHolons('RequiresOperator')) {
      const key = await operator.key();
      if (!key || typeof operations?.[key] !== 'function') throw new Error(`Missing required Visualizer Operator binding: ${key}`);
    }
  }

  /** Offered declarations must also have materialized callables, independent of slot use. */
  async admitOffered(element: HTMLElement, selected: HolonReference): Promise<void> {
    const bindings = (element as HTMLElement & { visualizerOperations?: Record<string, unknown> }).visualizerOperations;
    for (const operator of await selected.relatedHolons('OffersOperator')) {
      const key = await operator.key();
      if (!key || typeof bindings?.[key] !== 'function') throw new Error(`Missing offered Visualizer Operator binding: ${key}`);
    }
  }

  /** A presentation may supply its own transaction-scoped materialization cache;
   * verified module identity remains shared across the experience's realizations. */
  async realize(selectedVisualizer: HolonReference, cache = this.cache): Promise<unknown> {
    const module = await cache.get(selectedVisualizer);
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
