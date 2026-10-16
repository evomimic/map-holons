export const APPLY_NODE_PRESENTATION: 'ApplyNodePresentation.VisualizerOperator';
export type NodeCapability = 'inspect' | 'navigate-singular' | 'navigate-plural' | 'present-active-collection';
export interface NodePresentation { width: number; height: number; requiredCapabilities: NodeCapability[]; }
export function validateNodePresentation(input: NodePresentation): NodePresentation;
export function applyNodePresentation(element: { visualizerOperations?: Record<string, (input: NodePresentation) => unknown> }, input: NodePresentation): unknown;
