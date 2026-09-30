import type { ContextRequestResult } from './context-host';

/** Presentation requests share the host's explicit applied/refused/support outcomes. */
export type PresentationRequestResult = ContextRequestResult;
export type MaximizeOperation = 'maximize' | 'restore';
export type InspectorRegion = 'properties' | 'collections';

/** Target identity is the retained occurrence element, never its semantic Holon. */
export interface OccurrenceAttentionRequest {
  operation: MaximizeOperation;
  target: HTMLElement;
}
