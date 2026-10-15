import type { NodePresentation } from '../../../../conductora/visualizer-src/node-presentation.js';
export type NodeInspectorAllocation = NodePresentation;
/** Operator identity is resolved from the selected slot's required contract. */
export interface NodeInspectorParticipant {
  readonly visualizerOperations: Record<string, (input: NodePresentation) => unknown>;
  getNodeInspectorAllocation?(): NodeInspectorAllocation | undefined;
}
