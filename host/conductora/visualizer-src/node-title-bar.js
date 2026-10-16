/** Shared adapter from a Node's occurrence authority to its selected Title Bar.
 * Membership is read from the shared schema contract by the composition runtime.
 */
export function createNodeTitleBar(context, options) {
  const title = context.childVisualizers?.get('titleBar');
  const actions = context.childVisualizers?.get('actions');
  const element = document.createElement('header');
  element.style.cssText = 'display:grid;grid-template-rows:minmax(0,1fr) minmax(0,1fr);min-width:0;min-height:0;overflow:hidden;';
  element.append(title);
  if (actions) element.append(actions);
  if (!title?.visualizerOperations?.['ApplyTitlePresentation.VisualizerOperator']) throw new Error('Node requires an admitted selected Title Bar');
  let state = {}, information;
  const operations = {
    ...context.presentationOperators,
    'ExploreOccurrence.VisualizerOperator': () => options.explore?.(),
    'CloseOccurrence.VisualizerOperator': () => options.close?.(),
    'RestoreNodePresentation.VisualizerOperator': () => {
      options.restoreLocal?.();
      if (state.collapsed) return options.restore?.();
      return { status: 'already-satisfied' };
    },
    'ExpandRestoreOccurrence.VisualizerOperator': () => state.collapsed ? options.restore?.() : options.maximize?.(state.maximized ? 'restore' : 'maximize'),
    'InspectOccurrence.VisualizerOperator': invoker => information?.(invoker),
  };
  const publish = () => {
    const binding = title.visualizerOperations['ApplyTitlePresentation.VisualizerOperator'];
    const actionsState = {
      ...context.presentationActionStates,
      'ExploreOccurrence.VisualizerOperator': { available: !!state.canExplore, reason: 'Explore is unavailable in this context.' },
      'CloseOccurrence.VisualizerOperator': { available: !!state.canClose, reason: 'Close is unavailable in this context.' },
      'ExpandRestoreOccurrence.VisualizerOperator': { available: state.collapsed || !!state.canMaximize, reason: 'Expansion is unavailable in this context.', label: state.collapsed || state.maximized ? 'Restore' : 'Expand' },
      'InspectOccurrence.VisualizerOperator': { available: !!information, reason: 'Occurrence inspection is unavailable.' },
    };
    actions?.setOccurrenceActions?.(actionsState, invoke);
    return binding({ width: state.width ?? 800, height: Math.max(32, (state.height ?? 96) / 2), title: options.title, holonKey: options.holonKey,
      requiresRestore: !!state.requiresRestore });
  };
  const update = patch => {
    state = { ...state, ...patch };
    state.collapsed = state.vertical !== 'full-height' || state.horizontal !== 'full-width';
    state.requiresRestore = !!state.localExpanded || state.collapsed;
    publish();
  };
  // A refused action is observable; no DOM removal or nearest-parent targeting.
  const invoke = (key, invoker) => {
    if (!element.isConnected) throw new Error('Title Bar occurrence is retired');
    const operation = operations[key] ?? context.presentationOperators?.[key];
    if (typeof operation !== 'function') throw new Error(`Missing occurrence operation: ${key}`);
    return operation(invoker);
  };
  title.setOwnerInvocation(invoke);
  const status = document.createElement('span'); status.setAttribute('role', 'status'); status.hidden = true;
  element.append(status);
  return { element, titleControl: title, presentationStatus: status, operations,
    update, setInformation(handler) { information = handler; publish(); } };
}

/** Identity presentation; sibling ActionBar composition belongs to the Node. */
export default class NodeTitleBar extends HTMLElement {
  get visualizerOperations() { return { 'ApplyTitlePresentation.VisualizerOperator': input => this.apply(input) }; }
  setContext(context) {
    this.dataset.nodeTitleBar = 'true';
    this.setAttribute('role', 'banner');
    this.context = context;
    this.identity = document.createElement('button'); this.identity.type = 'button';
    this.identity.style.cssText = 'min-width:0;min-height:0;flex:1 1 0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;text-align:left;border:0;background:transparent;color:inherit;font:inherit;';
    this.identity.addEventListener('click', () => this.invoke('RestoreNodePresentation.VisualizerOperator', this.identity));
    this.style.cssText = 'display:flex;align-items:center;min-width:0;min-height:0;box-sizing:border-box;overflow:auto;position:relative;background:var(--dahn-action-surface-background);color:var(--dahn-action-text-color);font:inherit;';
    this.tabIndex = -1;
    this.replaceChildren(this.identity);
    this.ready = Promise.resolve();
  }
  setOwnerInvocation(invoke) { this.invoke = invoke; }
  apply(input) {
    if (!Number.isFinite(input.width) || !Number.isFinite(input.height) || input.width < 80 || input.height < 32 || !input.title) throw new Error('Invalid Title Bar presentation');
    this.input = input;
    this.identity.textContent = input.requiresRestore ? input.holonKey || input.title : input.title; this.identity.title = input.title;
    this.identity.setAttribute('aria-label', `${input.requiresRestore ? 'Restore: ' : ''}${input.title}`);
    this.identity.setAttribute('aria-expanded', String(!input.requiresRestore));
    this.style.width = `${input.width}px`; this.style.height = `${input.height}px`;
    return { status: 'applied' };
  }
  getVisualizerComposition() { return []; }
}
