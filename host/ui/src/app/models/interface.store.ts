//import { Cell } from "./interface.cell";
import { Signal } from "@angular/core";
import { DeprecatedHolonSpace } from "./interface.space";

/**
 * Defines the absolute minimum properties that every store instance will have.
 * This provides a base type for the StoreManager cache.
 */
export interface SignalStore {
  readonly space: Signal<DeprecatedHolonSpace | undefined>;
  readonly loading: Signal<boolean>;
  onDestroy?(): void; // Optional lifecycle hook for cleanup
  // TODO: add other truly universal signals or methods here if they exist.
}

/**
 * Defines the shape of the state object for any store.
 * Individual stores will extend this with their specific properties.
 */
export interface StoreState {
 space:DeprecatedHolonSpace | undefined
 loading: boolean;
}