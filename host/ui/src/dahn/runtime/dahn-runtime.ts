import type { DahnTarget } from '../contracts/targets';

/**
 * Minimal DAHN runtime interface for Phase 0.
 */
export interface DahnRuntime {
  open(target: DahnTarget): Promise<void>;
}
