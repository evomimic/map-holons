import { actionIcon } from './action-icons.js';
import { validateNodePresentation } from './node-presentation.js';

/** Immediate regions for exactly four private modes; no content-derived budgets. */
export function inspectorLayout(input, titleHeight = 48, mode = 'properties', requestedPane) {
  const grant = validateNodePresentation(input, mode);
  const inspect = mode === 'properties', plural = mode === 'collection', singular = inspect || mode === 'relationship';
  const scale = titleHeight / 48;
  const title = mode === 'compact' ? grant.height : Math.min(grant.height, titleHeight);
  const pane = requestedPane === 'properties' && inspect ? 'properties' : undefined;
  const subjectActions = inspect && !pane ? 48 * scale : 0;
  const groupHeading = !pane && (inspect || plural) ? 32 * scale : 0;
  const remaining = grant.height - title - groupHeading - subjectActions;
  const heading = inspect ? Math.min(32 * scale, remaining) : 0;
  const tabs = inspect && !pane ? Math.min(56 * scale, remaining) : 0;
  const actions = inspect && !pane ? Math.min(32 * scale, remaining - tabs) : 0;
  const body = mode === 'compact' || plural ? 0 : remaining - tabs;
  return { ...grant, mode, pane, inspect, singular, plural, details: inspect, groupHeading, heading, title, subjectActions, actions, body,
    collection: mode === 'compact' || mode === 'relationship' ? 0 : plural ? remaining : tabs,
    showRail: singular && !pane };
}
export function boundedRegion(element, visible, label) {
  if (!element) return;
  element.hidden = !visible; element.inert = !visible;
  Object.assign(element.style, { minWidth: '0', minHeight: '0', maxWidth: '100%', maxHeight: '100%', boxSizing: 'border-box', overflow: 'auto' });
  if (visible) { element.tabIndex = 0; element.setAttribute('aria-label', label); }
}
/** Both semantic bindings pass the same immediate parts to the same geometry owner. */
export function applyInspectorGeometry(parts, layout) {
  const { root, title, body, heading, groupHeading, toggle, properties, filters, rail, collections, subjectActions } = parts;
  // Logical group membership is shared by both Node bindings. Inspection spans
  // adjacent owned regions; Collection already has a containing region.
  for (const [group, members] of Object.entries({ title: [title], inspection: [groupHeading, subjectActions, body], collection: [collections] })) {
    for (const member of members) if (member) member.dataset.inspectorGroup = group;
  }
  Object.assign(root.style, { display: 'grid', width: `${layout.width}px`, height: `${layout.height}px`, boxSizing: 'border-box', gap: '0',
    gridTemplateColumns: 'minmax(0, 1fr)', gridTemplateRows: `${layout.title}px ${layout.groupHeading}px ${layout.subjectActions}px ${layout.body}px ${layout.collection}px` });
  Object.assign(title.style, { gridRow: '1', height: `${layout.title}px`, minWidth: '0', minHeight: '0' });
  const show = (element, visible, display, label) => {
    boundedRegion(element, visible, label);
    if (element) element.style.display = visible ? display : 'none';
  };
  show(groupHeading, layout.groupHeading > 0, 'flex', 'Properties, Actions & Relationships');
  if (groupHeading) Object.assign(groupHeading.style, { gridRow: '2', gridColumn: '1',
    background: layout.inspect ? 'color-mix(in srgb, var(--dahn-panel-surface-background) 72%, var(--dahn-action-surface-background))' : 'transparent' });
  if (root.inspectionLabel) root.inspectionLabel.hidden = !layout.inspect;
  show(body, layout.body > 0, 'grid', 'Node navigation and details');
  show(subjectActions, layout.inspect && !layout.pane, 'block', 'Holon actions');
  if (subjectActions) Object.assign(subjectActions.style, { gridRow: '3', height: `${layout.subjectActions}px`, overflow: 'auto' });
  show(heading, layout.heading > 0, 'flex', 'Properties');
  show(properties, layout.details, 'flex', 'Properties — scroll for more');
  show(filters, layout.details && layout.actions > 0, 'flex', 'Relationship discovery');
  show(rail, layout.showRail, 'flex', 'Single-valued relationships — scroll for more');
  show(collections, layout.collection > 0, 'flex', 'Collections — scroll for more');
  root.dataset.inspectorMode = layout.mode;
  root.dataset.propertiesExpanded = String(layout.details);
  if (toggle) {
    toggle.setAttribute('aria-expanded', String(layout.details));
    toggle.disabled = layout.inspect;
    toggle.hidden = layout.inspect;
    toggle.textContent = layout.inspect ? 'Properties, Actions & Relationships' : '‹ Properties, Actions & Relationships';
    toggle.setAttribute('aria-label', 'Properties, Actions & Relationships');
    toggle.style.cursor = layout.inspect ? 'default' : 'pointer';
    toggle.title = layout.inspect ? 'Properties, Actions & Relationships' : 'Show Properties, Actions & Relationships';
  }
  Object.assign(body.style, { gridRow: '4', rowGap: '0', gridColumn: '1',
    gridTemplateColumns: layout.inspect && layout.showRail ? 'minmax(0, 1fr) minmax(0, min(40%, 14rem))' : 'minmax(0, 1fr)',
    gridTemplateRows: layout.inspect ? `${layout.actions}px ${layout.heading}px minmax(0, 1fr)` : 'minmax(0, 1fr)' });
  if (heading) Object.assign(heading.style, { gridRow: '2', gridColumn: '1',
    fontSize: '.9375em', fontWeight: '600', background: 'transparent',
    borderBottom: '1px solid var(--dahn-slot-border-color)' });
  if (filters) Object.assign(filters.style, { gridRow: '1', gridColumn: '1 / -1' });
  Object.assign(properties.style, { gridRow: '3', gridColumn: '1', flexDirection: 'column' });
  Object.assign(rail.style, { gridRow: layout.inspect ? '2 / 4' : '1', gridColumn: layout.inspect ? '2' : '1',
    justifySelf: 'end', width: '100%', maxWidth: layout.inspect ? '14rem' : '100%' });
  Object.assign(collections.style, { gridRow: '5', gridColumn: '1', flexDirection: 'column' });
  const groupBorder = 'var(--dahn-slot-border-width) var(--dahn-slot-border-style) var(--dahn-slot-border-color)';
  for (const member of [subjectActions, body]) if (member) Object.assign(member.style, {
    borderInline: layout.inspect && !layout.pane ? groupBorder : '0',
    background: layout.inspect && !layout.pane
      ? 'color-mix(in srgb, var(--dahn-panel-surface-background) 88%, var(--dahn-action-surface-background))'
      : 'transparent',
  });
  body.style.borderBottom = layout.inspect && !layout.pane ? groupBorder : '0';
}

/** A visible, keyboard-accessible return action, rather than passive heading text. */
export function configureInspectionReturn(button) {
  button.textContent = '‹ Properties, Actions & Relationships';
  button.setAttribute('aria-label', 'Properties, Actions & Relationships');
  button.title = 'Show Properties, Actions & Relationships';
  button.style.cssText = 'text-align:left;flex:1;min-width:0;height:100%;box-sizing:border-box;cursor:pointer;font:inherit;font-weight:600;padding:0 .75rem;border:1px solid var(--dahn-slot-border-color);border-radius:var(--dahn-action-corner-radius);background:var(--dahn-action-surface-background);color:var(--dahn-action-text-color);';
}

/** Properties-only expansion stays within the current grant, never through Path. */
export function applyPaneControls(parts, layout, relayout) {
  const { root, heading, collectionTabs, collectionReturn } = parts;
  if (root.localPaneAttention && root.localPaneAttention !== layout.pane) root.localPaneAttention = undefined;
  root.dataset.paneAttention = layout.pane ?? '';
  const size = 32 * layout.title / 48;
  let button = root.propertiesPaneControl;
  if (!button) {
    button = root.propertiesPaneControl = document.createElement('button'); button.type = 'button';
    button.dataset.paneAttentionControl = 'properties';
  }
  if (button.parentElement !== heading) heading.append(button);
  const expanded = layout.pane === 'properties';
  const label = `${expanded ? 'Restore' : 'Expand'} Properties pane`;
  button.title = label; button.setAttribute('aria-label', label); button.setAttribute('aria-pressed', String(expanded));
  button.replaceChildren(actionIcon('ExpandRestoreOccurrence.VisualizerOperator', expanded));
  button.style.cssText = `display:flex;align-items:center;justify-content:center;flex:0 0 ${size}px;width:${size}px;height:${size}px;box-sizing:border-box;padding:4px;border:0;background:var(--dahn-action-surface-background);color:var(--dahn-action-text-color);cursor:pointer;`;
  button.hidden = !layout.inspect; button.inert = !layout.inspect;
  if (!layout.inspect) button.style.display = 'none';
  button.onclick = () => {
    root.localPaneAttention = expanded ? undefined : 'properties';
    relayout(); button.focus({ preventScroll: true });
  };
  if (collectionTabs) {
    // The Collection group uses its full width; its index owns overflow disclosure.
    collectionTabs.hidden = false; collectionTabs.inert = false;
    collectionTabs.style.paddingRight = '0px';
  }
  if (collectionReturn) {
    Object.assign(collectionReturn.style, { minHeight: `${size}px`, marginRight: '0px' });
    collectionReturn.inert = false;
  }
}

/** Stable group identity, distinct from the narrower Properties pane heading. */
export function createInspectionGroupHeader(root, returnToInspection) {
  const header = root.inspectionHeading = document.createElement('header');
  header.dataset.inspectionGroupHeader = 'true';
  header.style.cssText = 'display:flex;align-items:center;min-width:0;min-height:0;';
  const button = root.propertiesToggle = document.createElement('button'); button.type = 'button';
  configureInspectionReturn(button);
  button.addEventListener('click', () => { returnToInspection(); root.propertiesHeading?.focus({ preventScroll: true }); });
  const label = root.inspectionLabel = document.createElement('span');
  label.textContent = 'Properties, Actions & Relationships';
  label.style.cssText = 'min-width:0;padding:0 .75rem;font-size:1em;font-weight:600;color:var(--dahn-canvas-text-color);';
  header.append(label, button); return header;
}
