import type { ActionNode } from './actions';
import type { CanvasApi } from './canvas';
import type { HolonViewAccess } from './holon-view';
import type { DahnTarget } from './targets';
import type { DahnTheme } from './themes';

/**
 * Minimal target classification metadata for Phase 0. Definitions stay local
 * and bootstrap-oriented for now, but must remain compatible with future
 * MAP-backed visualizer descriptor holons.
 */
export interface VisualizerTargetRule {
  kind:
    | 'holon-node'
    | 'action'
    | 'property'
    | 'relationship'
    | 'debug';
}

/**
 * Runtime/local representation of a visualizer descriptor.
 */
export interface VisualizerDefinition {
  id: string;
  displayName: string;
  version: string;
  componentTag: string;
  supportedTargets: VisualizerTargetRule[];
  load: () => Promise<void>;
}

/**
 * Common context passed into Web Component visualizers.
 */
export interface VisualizerContext {
  target: DahnTarget;
  holon: HolonViewAccess;
  actions: ActionNode[];
  theme: DahnTheme;
  canvas: CanvasApi;
}

/**
 * Common surface all DAHN visualizer elements must implement.
 */
export interface VisualizerElement extends HTMLElement {
  setContext(context: VisualizerContext): void;
}
