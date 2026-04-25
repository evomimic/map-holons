import type { DahnTarget } from '../contracts/targets';
import type { DahnRuntime } from './dahn-runtime';
import { DahnNotImplementedError } from './runtime-errors';

/**
 * Placeholder runtime implementation for PR 1.
 *
 * This class exists only to establish the seam that later PRs will implement.
 */
export class DefaultDahnRuntime implements DahnRuntime {
  async open(_target: DahnTarget): Promise<void> {
    throw new DahnNotImplementedError(
      'DefaultDahnRuntime.open() is not implemented in Phase 0 / PR 1',
    );
  }
}
