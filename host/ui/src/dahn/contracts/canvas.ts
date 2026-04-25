import type { DahnTheme } from './themes';
import type { DahnTarget } from './targets';

/**
 * Phase 0 canvas descriptor. The runtime currently supports a single visible
 * slot, but the contract leaves room for future expansion.
 */
export interface CanvasDescriptor {
  id: string;
  slots: ['primary'];
}

/**
 * Describes which visualizer should be mounted into which canvas slot.
 */
export interface VisualizerMountPlan {
  visualizerId: string;
  target: DahnTarget;
  slot: 'primary';
}

/**
 * Minimal canvas API for Phase 0.
 */
export interface CanvasApi {
  mountVisualizers(plan: VisualizerMountPlan[]): Promise<void>;
  clear(): void;
  setTheme(theme: DahnTheme): void;
}
