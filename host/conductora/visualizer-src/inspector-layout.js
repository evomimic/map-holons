import { validateNodePresentation } from './node-presentation.js';

/** Immediate regions for exactly four private modes; no content-derived budgets. */
export function inspectorLayout(input, titleHeight = 48, mode = 'properties') {
  const grant = validateNodePresentation(input, mode);
  const inspect = mode === 'properties', plural = mode === 'collection', singular = inspect || mode === 'relationship';
  const scale = titleHeight / 48;
  const title = Math.min(grant.height, titleHeight);
  const subjectActions = inspect ? 48 * scale : 0;
  const remaining = grant.height - title - subjectActions;
  const heading = inspect || plural ? Math.min(32 * scale, remaining) : 0;
  const tabs = inspect ? Math.min(56 * scale, remaining) : 0;
  const actions = inspect ? Math.min(32 * scale, remaining - tabs) : 0;
  const body = mode === 'compact' ? 0 : plural ? heading : remaining - tabs;
  return { ...grant, mode, inspect, singular, plural, details: inspect, heading, title, subjectActions, actions, body,
    collection: mode === 'compact' || mode === 'relationship' ? 0 : plural ? remaining - heading : tabs,
    showRail: singular };
}
export function boundedRegion(element, visible, label) {
  if (!element) return;
  element.hidden = !visible; element.inert = !visible;
  Object.assign(element.style, { minWidth: '0', minHeight: '0', maxWidth: '100%', maxHeight: '100%', boxSizing: 'border-box', overflow: 'auto' });
  if (visible) { element.tabIndex = 0; element.setAttribute('aria-label', label); }
}
/** Both semantic bindings pass the same immediate parts to the same geometry owner. */
export function applyInspectorGeometry(parts, layout) {
  const { root, title, body, heading, toggle, properties, filters, rail, collections, subjectActions } = parts;
  Object.assign(root.style, { display: 'grid', width: `${layout.width}px`, height: `${layout.height}px`, boxSizing: 'border-box', gap: '0',
    gridTemplateColumns: 'minmax(0, 1fr)', gridTemplateRows: `${layout.title}px ${layout.subjectActions}px ${layout.body}px ${layout.collection}px` });
  Object.assign(title.style, { gridRow: '1', height: `${layout.title}px`, minWidth: '0', minHeight: '0' });
  const show = (element, visible, display, label) => {
    boundedRegion(element, visible, label);
    if (element) element.style.display = visible ? display : 'none';
  };
  show(body, layout.body > 0, 'grid', 'Node navigation and details');
  show(subjectActions, layout.inspect, 'block', 'Holon actions');
  if (subjectActions) Object.assign(subjectActions.style, { gridRow: '2', height: `${layout.subjectActions}px`, overflow: 'auto' });
  show(heading, layout.inspect || layout.mode === 'collection', 'flex', 'Properties');
  show(properties, layout.details, 'flex', 'Properties — scroll for more');
  show(filters, layout.details && layout.actions > 0, 'flex', 'Relationship discovery');
  show(rail, layout.showRail, 'flex', 'Single-valued relationships — scroll for more');
  show(collections, layout.collection > 0, 'flex', 'Collections — scroll for more');
  root.dataset.inspectorMode = layout.mode;
  root.dataset.propertiesExpanded = String(layout.details);
  if (toggle) {
    toggle.setAttribute('aria-expanded', String(layout.details));
    toggle.disabled = layout.inspect;
    toggle.textContent = layout.inspect ? 'Properties' : '‹ Properties';
    toggle.style.cursor = layout.inspect ? 'default' : 'pointer';
    toggle.title = layout.inspect ? 'Properties' : 'Show Properties';
  }
  Object.assign(body.style, { gridRow: '3', gridColumn: '1',
    gridTemplateColumns: layout.inspect && layout.showRail ? 'minmax(0, 1fr) minmax(0, min(40%, 14rem))' : 'minmax(0, 1fr)',
    gridTemplateRows: layout.inspect ? `${layout.heading}px ${layout.actions}px minmax(0, 1fr)` : 'minmax(0, 1fr)' });
  if (heading) Object.assign(heading.style, { gridRow: '1', gridColumn: '1 / -1' });
  if (filters) Object.assign(filters.style, { gridRow: '2', gridColumn: '1 / -1' });
  Object.assign(properties.style, { gridRow: '3', gridColumn: '1', flexDirection: 'column' });
  Object.assign(rail.style, { gridRow: layout.inspect ? '3' : '1', gridColumn: layout.inspect ? '2' : '1',
    justifySelf: 'end', width: '100%', maxWidth: layout.inspect ? '14rem' : '100%' });
  Object.assign(collections.style, { gridRow: '4', gridColumn: '1', flexDirection: 'column' });
}

/** A visible, keyboard-accessible return action, rather than passive heading text. */
export function configurePropertiesReturn(button) {
  button.textContent = '‹ Properties';
  button.setAttribute('aria-label', 'Properties');
  button.title = 'Show Properties';
  button.style.cssText = 'text-align:left;flex:1;min-width:0;height:100%;box-sizing:border-box;cursor:pointer;font:inherit;font-weight:600;padding:0 .75rem;border:1px solid var(--dahn-slot-border-color);border-radius:var(--dahn-action-corner-radius);background:var(--dahn-action-surface-background);color:var(--dahn-action-text-color);';
}
