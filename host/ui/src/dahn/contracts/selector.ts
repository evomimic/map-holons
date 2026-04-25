import type { ActionNode } from './actions';
import type { CanvasDescriptor, VisualizerMountPlan } from './canvas';
import type { HolonViewAccess } from './holon-view';
import type { DahnTarget } from './targets';
import type { VisualizerDefinition } from './visualizers';

/**
 * Input to the Phase 0 TypeScript-side presentation resolution seam.
 *
 * This is intentionally limited to runtime resolution inputs and must remain
 * future-compatible with Rust-side semantic recommendation feeding into the
 * TypeScript layer later.
 */
export interface SelectorInput {
  target: DahnTarget;
  holon: HolonViewAccess;
  actions: ActionNode[];
  availableVisualizers: VisualizerDefinition[];
  canvas: CanvasDescriptor;
}

/**
 * Output mount plan selected for the current runtime.
 */
export interface SelectorOutput {
  visualizers: VisualizerMountPlan[];
}

/**
 * Phase 0 TypeScript-side selector/resolution seam.
 */
export interface SelectorFunction {
  select(input: SelectorInput): SelectorOutput;
}
