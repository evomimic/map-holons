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
