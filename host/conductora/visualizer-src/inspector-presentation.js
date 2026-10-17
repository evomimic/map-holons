/** Node-local full-content memory. Parent requests never encode private layout. */
export function initialInspectorPresentation() {
  return { mode: 'properties', remembered: 'properties' };
}
export function inspectorPresentation(state, request) {
  if (request === undefined) return state;
  if (request === 'inspect' || request === 'collection-context') return { ...state, mode: state.remembered };
  if (request === 'relationship' || request === 'compact') return { ...state, mode: request };
  throw new Error('Unknown Node presentation request');
}
/** Local switching updates memory without expanding a parent-compressed occurrence. */
export function selectInspectorView(state, view) {
  if (!['properties', 'collection'].includes(view)) throw new Error('Unknown Inspector view');
  return { mode: ['relationship', 'compact'].includes(state.mode) ? state.mode : view, remembered: view };
}
