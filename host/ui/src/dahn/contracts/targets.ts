import type { HolonReference } from '../deps/map-sdk';

/**
 * Runtime-level target handle used to identify the holon DAHN should open.
 *
 * The underlying HolonReference is already transaction-bound by the public SDK.
 */
export interface DahnTarget {
  reference: HolonReference;
}
