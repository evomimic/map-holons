import type { ActionNode } from './actions';
import type { CanvasApi } from './canvas';
import type { HolonViewAccess } from './holon-view';
import type { DahnTarget } from './targets';
import type { DahnTheme } from './themes';
import type { HolonReference } from '../deps';

/**
 * Minimal target classification metadata for Phase 0. Definitions stay local
 * and bootstrap-oriented for now, but must remain compatible with future
 * MAP-backed visualizer descriptor holons.
 */
export interface VisualizerTargetRule {
  kind:
    | 'canvas'
    | 'collection'
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
  /**
   * Local executable-registry identity. This is not a MAP semantic identity.
   */
  id: string;
  /**
   * Stable MAP implementation key, when this definition realizes a bundled
   * VisualizerImplementation Holon.
   */
  implementationKey?: string;
  displayName: string;
  version: string;
  componentTag: string;
  supportedTargets: VisualizerTargetRule[];
  load: () => Promise<void>;
}

/** Execution runtimes currently described by the DAHN schema. */
export type VisualizerImplementationRuntimeKind = 'TypeScript' | 'Rust';

/**
 * A Rust-selected realization request. The resolver trusts this pairing and
 * only maps the supplied implementation key to locally executable code.
 */
export interface VisualizerImplementationResolution {
  selectedVisualizer: HolonReference;
  implementation: HolonReference;
  runtime: VisualizerImplementationRuntimeKind;
  implementationKey?: string;
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
