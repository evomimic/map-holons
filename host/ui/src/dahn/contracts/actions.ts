import type { HolonReference } from '../deps';

/**
 * Minimal action hierarchy node used to present dances.
 */
export interface ActionNode {
  id: string;
  kind: 'action' | 'group';
  label: string;
  dance?: HolonReference;
  children?: ActionNode[];
}

/** Source-independent input to PresentActions. Authority is captured by the owner. */
export type PresentedAction = { id: string; label: string } & (
  { kind: 'group'; children: PresentedAction[] } |
  { kind: 'action'; icon?: string; available: boolean; reason?: string; activate(invoker: HTMLElement): unknown }
);

export interface ActionsPresentation {
  readonly actions: readonly PresentedAction[];
  /** The owner reserves space for the complete supplied action set when false. */
  readonly overflow?: boolean;
}
