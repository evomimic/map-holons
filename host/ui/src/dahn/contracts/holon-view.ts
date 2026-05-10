import type { ActionNode } from './actions';
import type { HolonReference } from '../deps';

/**
 * DAHN uses the public SDK's bound holon handle directly rather than defining a
 * parallel DAHN-owned holon access surface.
 */
export type HolonViewAccess = HolonReference;

/**
 * Composite object passed from the access adapter into the runtime.
 */
export interface HolonViewContext {
  holon: HolonViewAccess;
  actions: ActionNode[];
}
