import type { VisualizerRegistry } from './visualizer-registry';
import { BUILTIN_VISUALIZER_DEFINITIONS } from '../visualizers/builtins';

export function registerBuiltInVisualizers(
  registry: VisualizerRegistry,
): void {
  for (const definition of BUILTIN_VISUALIZER_DEFINITIONS) {
    registry.register(definition);
  }
}
