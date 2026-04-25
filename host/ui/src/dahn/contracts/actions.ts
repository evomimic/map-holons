import type { DanceDescriptorHandle } from './holon-view';

/**
 * Minimal action hierarchy node used to present dances.
 */
export interface ActionNode {
  id: string;
  kind: 'action' | 'group';
  label: string;
  dance?: DanceDescriptorHandle;
  children?: ActionNode[];
}
