export const APPLY_NODE_PRESENTATION: 'ApplyNodePresentationV2.VisualizerOperator';
export type NodePresentationRequest = 'inspect' | 'collection-context' | 'relationship' | 'compact';
export interface NodePresentation { width: number; height: number; presentation?: NodePresentationRequest; }
export function validateNodePresentation(input: NodePresentation, mode?: string): NodePresentation;
export function applyNodePresentation(element: { visualizerOperations?: Record<string, (input: NodePresentation) => unknown> }, input: NodePresentation): unknown;
export function assignedNodePresentation(element: object): NodePresentation | undefined;
