import type { VisualizerDefinition } from '../contracts/visualizers';
import type { VisualizerRegistry } from './visualizer-registry';

export class DefaultVisualizerRegistry implements VisualizerRegistry {
  private readonly definitions = new Map<string, VisualizerDefinition>();
  private readonly definitionsByImplementationKey = new Map<
    string,
    VisualizerDefinition
  >();
  private readonly loadedIds = new Set<string>();
  private readonly pendingLoads = new Map<string, Promise<VisualizerDefinition>>();

  register(definition: VisualizerDefinition): void {
    const existing = this.definitions.get(definition.id);
    if (existing !== undefined && existing !== definition) {
      throw new Error(
        `Visualizer definition '${definition.id}' is already registered`,
      );
    }

    if (definition.implementationKey !== undefined) {
      const existingByKey = this.definitionsByImplementationKey.get(
        definition.implementationKey,
      );
      if (existingByKey !== undefined && existingByKey !== definition) {
        throw new Error(
          `Visualizer implementation key '${definition.implementationKey}' is already registered`,
        );
      }
    }

    this.definitions.set(definition.id, definition);

    if (definition.implementationKey !== undefined) {
      this.definitionsByImplementationKey.set(
        definition.implementationKey,
        definition,
      );
    }
  }

  get(id: string): VisualizerDefinition | undefined {
    return this.definitions.get(id);
  }

  getByImplementationKey(key: string): VisualizerDefinition | undefined {
    return this.definitionsByImplementationKey.get(key);
  }

  list(): VisualizerDefinition[] {
    return [...this.definitions.values()];
  }

  async ensureLoaded(id: string): Promise<VisualizerDefinition> {
    const definition = this.definitions.get(id);
    if (definition === undefined) {
      throw new Error(`Unknown visualizer definition '${id}'`);
    }

    if (this.loadedIds.has(id)) {
      return definition;
    }

    const existingLoad = this.pendingLoads.get(id);
    if (existingLoad !== undefined) {
      return existingLoad;
    }

    const loadPromise = (async () => {
      if (customElements.get(definition.componentTag) === undefined) {
        await definition.load();
      }

      this.loadedIds.add(id);
      return definition;
    })();

    this.pendingLoads.set(id, loadPromise);

    try {
      return await loadPromise;
    } finally {
      this.pendingLoads.delete(id);
    }
  }

  async ensureImplementationLoaded(key: string): Promise<VisualizerDefinition> {
    const definition = this.getByImplementationKey(key);
    if (definition === undefined) {
      throw new Error(`Unknown visualizer implementation key '${key}'`);
    }

    return this.ensureLoaded(definition.id);
  }
}
