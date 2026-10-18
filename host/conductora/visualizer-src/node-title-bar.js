/** Shared adapter from a Node's occurrence authority to its selected Title Bar.
 * Membership is read from the shared schema contract by the composition runtime.
 */
export function createNodeTitleBar(context, options) {
  const title = context.childVisualizers?.get('titleBar');
  const actions = context.childVisualizers?.get('visualizerActions');
  if (!title?.visualizerOperations?.['ApplyTitlePresentation.VisualizerOperator']) throw new Error('Node requires an admitted selected Title Bar');
  const element = document.createElement('header');
  element.dataset.nodeTitleHeader = 'true';
  element.style.cssText = 'display:flex;align-items:center;min-width:0;min-height:0;overflow:auto;';
  element.append(title);
  title.style.flex = '1 1 0'; title.style.minWidth = '0';
  if (actions) {
    actions.style.flex = '0 1 60%'; actions.style.minWidth = '44px'; actions.style.maxWidth = '60%';
    element.append(actions);
  }
  let state = {}, information, informationName;
  const operations = {
    ...context.presentationOperators,
    'ExploreOccurrence.VisualizerOperator': () => options.explore?.(),
    'CloseOccurrence.VisualizerOperator': () => options.close?.(),
    'RestoreNodePresentation.VisualizerOperator': () => {
      if (state.collapsed) return options.restore?.();
      options.restoreLocal?.();
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
      'InspectOccurrence.VisualizerOperator': { available: !!information, reason: 'Occurrence inspection is unavailable.', label: informationName || 'Show Visualizer' },
    };
    const width = state.width ?? 800;
    const scale = Math.max(1, (parseFloat(getComputedStyle(element).fontSize) || 16) / 16);
    // Reserve the complete standard action set, including its inspection badge.
    // Reduced presentations retain only Close; there is no partial icon set.
    const fullActionsWidth = 240 * scale;
    const reduced = ['relationship', 'compact'].includes(state.mode) || width < fullActionsWidth + 80 * scale;
    element.dataset.titleActions = reduced ? 'close-only' : 'all';
    actions?.setOccurrenceActions?.(actionsState, invoke, reduced ? 'close-only' : 'all');
    const height = Math.max(32, state.height ?? 48);
    const stacked = state.mode === 'compact' && width < 320 * scale && height >= 64 * scale;
    const actionsHeight = stacked ? Math.min(48 * scale, height - 32 * scale) : height;
    const actionWidth = actions ? (reduced ? Math.min(48 * scale, width - 1) : fullActionsWidth) : 0;
    const titleWidth = stacked ? width : Math.max(1, width - actionWidth);
    const titleHeight = stacked && actions ? height - actionsHeight : height;
    element.dataset.titleLayout = stacked ? 'stacked' : 'inline';
    element.style.background = 'var(--dahn-action-surface-background)';
    Object.assign(element.style, { display: 'grid', gridTemplateColumns: stacked ? 'minmax(0, 1fr)' : `minmax(0, 1fr) ${actionWidth}px`,
      gridTemplateRows: stacked && actions ? `${actionsHeight}px minmax(0, 1fr)` : 'minmax(0, 1fr)', alignItems: 'stretch' });
    Object.assign(title.style, { gridRow: stacked && actions ? '2' : '1', gridColumn: '1', minHeight: '0' });
    if (actions) Object.assign(actions.style, { gridRow: '1', gridColumn: stacked ? '1' : '2', justifySelf: 'end',
      width: `${actionWidth}px`, maxWidth: `${actionWidth}px`, height: `${actionsHeight}px`, minWidth: '0' });
    const result = binding({ width: titleWidth, height: titleHeight, title: options.title, holonKey: options.holonKey,
      requiresRestore: !!state.requiresRestore });
    return result;
  };
  const update = patch => {
    state = { ...state, ...patch };
    state.requiresRestore = !!state.collapsed;
    publish();
  };
  // A refused action is observable; no DOM removal or nearest-parent targeting.
  const invoke = (key, invoker) => {
    if (!element.isConnected) throw new Error('Title Bar occurrence is retired');
    const operation = operations[key] ?? context.presentationOperators?.[key];
    if (typeof operation !== 'function') throw new Error(`Missing occurrence operation: ${key}`);
    return operation(invoker);
  };
  // Compact gives this entire region to identity/Restore. The Close component
  // overlays it; interactive descendants retain their own invocation authority.
  element.addEventListener('click', event => {
    if (state.mode !== 'compact' || event.defaultPrevented) return;
    if (event.composedPath().some(target => target instanceof Element && target.matches('button, a, input, select, textarea, [role="button"]'))) return;
    invoke('RestoreNodePresentation.VisualizerOperator', element);
  });
  title.setOwnerInvocation(invoke);
  const status = document.createElement('span'); status.setAttribute('role', 'status'); status.hidden = true;
  element.append(status);
  return { element, actionsControl: actions, titleControl: title, presentationStatus: status, operations,
    update, setInformation(handler, displayName) { information = handler; informationName = displayName; publish(); } };
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
    if (!Number.isFinite(input.width) || !Number.isFinite(input.height) || input.width < 1 || input.height < 32 || !input.title) throw new Error('Invalid Title Bar presentation');
    this.input = input;
    this.identity.textContent = input.requiresRestore ? input.holonKey || input.title : input.title; this.identity.title = input.title;
    this.identity.setAttribute('aria-label', `${input.requiresRestore ? 'Restore: ' : ''}${input.title}`);
    this.identity.setAttribute('aria-expanded', String(!input.requiresRestore));
    layoutTitleIdentity(this, this.identity, input);
    return { status: 'applied' };
  }
  getVisualizerComposition() { return []; }
}

/** Identity owns its complete grant, including the Restore hit target. */
export function layoutTitleIdentity(element, identity, input) {
  Object.assign(element.style, { width: `${input.width}px`, height: `${input.height}px` });
  Object.assign(identity.style, { width: '100%', height: '100%', boxSizing: 'border-box',
    fontSize: '1.125em', fontWeight: '700',
    whiteSpace: input.requiresRestore ? 'normal' : 'nowrap', overflowWrap: 'anywhere',
    textOverflow: input.requiresRestore ? 'clip' : 'ellipsis' });
}
