import type { VisualizerDefinition } from '../contracts/visualizers';

export interface VisualizerRegistry {
  register(definition: VisualizerDefinition): void;
  get(id: string): VisualizerDefinition | undefined;
  list(): VisualizerDefinition[];
  ensureLoaded(id: string): Promise<VisualizerDefinition>;
}
