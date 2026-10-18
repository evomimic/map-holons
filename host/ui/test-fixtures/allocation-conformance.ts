import { composeNodeActions } from '../src/dahn/runtime/node-actions';
import { bindVisualizerInformationControl } from '../src/dahn/runtime/visualizer-information-control';
import { composeNodeTitle } from '../src/dahn/runtime/title-composition';
import { MaterializedVisualizerRuntime } from '../src/dahn/runtime/materialized-visualizer-runtime';
import { MaterializedVisualizerCache } from '../src/dahn/runtime/materialized-visualizer-cache';
import { defineCustomElementOnce } from '../src/dahn/visualizers/define-custom-element-once';
import { applyNodePresentation, participationProfile } from '../../conductora/visualizer-src/node-presentation.js';

/** Browser fixture backend supplies explicit semantic selections. Rust compatibility
 * is tested separately against the real reference layer; this fixture exercises
 * the production materialization, composition, admission and invocation path.
 */
const schema = await (await fetch('/schema.json')).json();
const records = new Map<string, any>(schema.holons.map((record: any) => [record.key, record]));
const references = new Map<string, any>();
function ref(key: string): any {
  if (!references.has(key)) references.set(key, { key: async () => key,
    propertyValue: async (name: string) => ({ StringValue: records.get(key)?.properties?.[name] ?? key }),
    relatedHolons: async (name: string) => (records.get(key)?.relationships?.find((edge: any) => edge.name === name)?.target ?? []).map((target: any) => ref(target.$ref)) });
  return references.get(key);
}
const subject = ref('allocation-fixture-subject');
let titleChoice = 'NodeTitleBar.TitleBarVisualizer';
const transaction: any = {
  selectVisualizer: async (request: any) => {
    const kind = request.requestedKind;
    const key = kind === 'titleBar' ? titleChoice : kind === 'actionBar' ? 'GenericActions.ActionBarVisualizer' : `${(await request.subject.key()).split('PresentationAction')[0]}Presentation.ActionVisualizer`;
    return { selected: ref(key) };
  },
  selectVisualizerUsage: async () => ({ usage: ref('fixture-usage'), reportSession: 'browser-fixture' }),
};
const runtime = new MaterializedVisualizerRuntime(new MaterializedVisualizerCache({ materialize: async selected => {
  const record = records.get(await selected.key());
  const implementation = records.get(record.relationships.find((edge: any) => edge.name === 'ImplementedBy').target[0].$ref);
  const filename = implementation.properties.VisualizerImplementationKey.split('.').at(-1);
  return { source: await (await fetch(`/resources/${filename}.js`)).text(), format: 'ESModule', entrypoint: 'default' };
} }));
let serial = 0;
export async function mount(kind: string, mode: string, textScale = 1, activeCollection = true, budget = 'minimum', reset = true, holonKey = 'Fixture.Subject', collectionLabels: string[] = []) {
  if (reset) document.body.replaceChildren();
  titleChoice = kind === 'loader' ? 'CompactTitleBar.TitleBarVisualizer' : 'NodeTitleBar.TitleBarVisualizer';
  const owner = ref(kind === 'generic' ? 'HolonInspector.NodeVisualizer' : 'LoadHolons.NodeVisualizer');
  const Constructor = await runtime.realize(owner) as CustomElementConstructor;
  const node: any = document.createElement(defineCustomElementOnce(`fixture-${kind}`, Constructor));
  const PropertyMap = await runtime.realize(ref('DefaultPropertyMapVisualizer.PropertyMapVisualizer')) as CustomElementConstructor;
  const property: any = document.createElement(defineCustomElementOnce('fixture-properties', PropertyMap));
  const propertyChildren = new Map();
  for (const label of ['Key', 'Description', 'Type', 'Version', 'Status', 'Owner']) {
    const child = document.createElement('label');
    child.style.cssText = 'display:flex;flex-direction:column;font:inherit;';
    child.textContent = label;
    const input = document.createElement('input'); input.value = `retained-${++serial}`; input.style.font = 'inherit';
    child.append(input); propertyChildren.set(label, child);
  }
  property.setContext({ title: 'Properties', childVisualizers: propertyChildren });
  await bindVisualizerInformationControl(property, transaction, subject, ref('HolonInspector.NodeVisualizer'), ref('HolonInspector.PropertyMapSlot'), ref('DefaultPropertyMapVisualizer.PropertyMapVisualizer'));
  const subjectActions = [{ id: 'fixture-load', kind: 'action', label: 'Load Holons', dance: ref('LoadHolons.DanceType') }, { id: 'fixture-query', kind: 'action', label: 'Query Dance', dance: ref('Query.DanceType') }];
  const actions = await composeNodeActions(transaction, runtime, owner, { target: { reference: subject }, actions: subjectActions, theme: { reference: ref('fixture-theme') } } as any);
  const collection = document.createElement('nav'); collection.setAttribute('aria-label', 'Non-table Collection');
  collection.style.cssText = 'display:flex;flex-direction:column;min-height:0;min-width:0;overflow:auto;flex:1;';
  for (let index = 0; index < 30; index++) {
    const button = document.createElement('button'); button.textContent = `Open Collection member ${index}`;
    button.style.cssText = 'flex:0 0 auto;font:inherit;'; collection.append(button);
  }
  const context: any = { title: 'Holon: A long readable subject identity', holonKey, target: { reference: subject }, actions: [],
    theme: { reference: ref('fixture-theme') }, canvas: {}, nodeAffordances: { singularRelationships: ['Navigate singular relationship', 'Owned by', 'Described by', 'Version of', 'Source', 'Parent'].map(label => ({ label, relationship: {} })), collections: collectionLabels.map(label => ({ kind: 'relationship', label, relationship: {} })) }, collectionActivation: { activate: (affordance: any, _slot: any, publish: any) => publish({ state: 'resolved', affordance, content: collection }), dispose() {} }, relationshipDiscovery: { population: () => ({ state: 'populated', count: 1 }), subscribe: (listener: () => void) => { listener(); return () => {}; } }, activateRelationship: () => {},
    childVisualizers: new Map([['properties', property], ...(activeCollection ? [['collections', collection]] : []), ['actions', actions], ['visualizerActions', actions.controls]]) };
  const title = await composeNodeTitle(transaction, runtime, owner, context);
  context.childVisualizers.set('titleBar', title);
  node.setContext(context);
  node.setOccurrenceRestorationHandler(() => { node.dataset.restored = 'true'; node.dataset.restoreCount = String(Number(node.dataset.restoreCount || 0) + 1); });
  node.setOccurrenceExplorationHandler(() => {}); node.setOccurrenceClosureHandler(() => { node.dataset.closeCount = String(Number(node.dataset.closeCount || 0) + 1); });
  node.setVisualizerInformationHandler(() => {}, 'Fixture Node');
  node.style.fontSize = `${16 * textScale}px`;
  const profile = participationProfile(16 * textScale);
  const horizontal = mode === 'compact' ? 0 : mode === 'relationship' ? 1 : 2;
  const vertical = mode === 'compact' ? 0 : mode === 'relationship' ? 1 : 2;
  const grant = { width: profile.horizontal[budget][horizontal], height: profile.vertical[budget][vertical], presentation: mode === 'compact' || mode === 'relationship' ? mode : 'inspect' };
  document.body.append(node);
  await runtime.admitOffered(node, owner);
  applyNodePresentation(node, grant);
  if (mode === 'collection') node.setNodeOwnedCollection(collection);
  await new Promise(requestAnimationFrame); await new Promise(requestAnimationFrame);
  return { grant, bounds: node.getBoundingClientRect().toJSON() };
}
export function measure() {
  const node: any = document.body.firstElementChild;
  const visible = (element: HTMLElement) => !!element.getClientRects().length && getComputedStyle(element).visibility !== 'hidden';
  const failures: string[] = [];
  // Each binding supplies its immediate owned regions; conformance does not use
  // private region names to establish Operator eligibility.
  const regions: HTMLElement[] = node.dataset.dahnHolonInspector ? [node.titleBar.element, node.inspectionHeading, node.actionsVisualizer, node.body, node.collectionRegion, node.propertyViewer, node.actionBar, node.singleValueRail, node.collectionViewer]
    : [node.heading, node.inspectionHeading, node.actionsVisualizer, node.body, node.content, node.properties, node.collections, node.rail];
  for (const region of regions.filter(visible)) {
    const parent = region.parentElement!.getBoundingClientRect(), child = region.getBoundingClientRect();
    if (child.left < parent.left - 1 || child.top < parent.top - 1 || child.right > parent.right + 1 || child.bottom > parent.bottom + 1) failures.push(`${region.getAttribute('aria-label') || region.tagName} exceeds owner: ${JSON.stringify(child.toJSON())} / ${JSON.stringify(parent.toJSON())}`);
  }
  const titleActions = node.titleBar.actionsControl;
  const titleButtons = [...titleActions.querySelectorAll('button[data-action-id]')].filter(visible);
  const informationAction = titleActions.querySelector('[data-action-id="InspectOccurrence.VisualizerOperator"]');
  if (informationAction && informationAction.title !== 'Fixture Node') failures.push(`Incorrect Node information tooltip: ${informationAction.title}`);
  const reducedTitle = ['relationship', 'compact'].includes(node.dataset.inspectorMode);
  if (titleButtons.length !== (reducedTitle ? 1 : 4)) failures.push(`Expected ${reducedTitle ? 'Close only' : 'all four title actions'}; got ${titleButtons.length}`);
  if (reducedTitle && titleButtons[0]?.dataset.actionId !== 'CloseOccurrence.VisualizerOperator') failures.push('Reduced title does not expose Close');
  const moreTitleActions = titleActions.querySelector('[data-overflow-more]');
  if (moreTitleActions && visible(moreTitleActions)) failures.push('Title actions use partial overflow');
  for (const action of titleButtons) {
    const parent = titleActions.getBoundingClientRect(), child = action.getBoundingClientRect();
    if (child.left < parent.left - 1 || child.right > parent.right + 1) failures.push('Title action exceeds its allocation');
  }
  // Slot inspection controls must not cover the containing Node's standard actions.
  for (const bar of node.querySelectorAll('[data-dahn-node-actions]')) {
    const badge = bar.querySelector(':scope > [data-visualizer-information-control] button');
    if (!badge || !visible(badge)) continue;
    const a = badge.getBoundingClientRect();
    for (const action of bar.querySelectorAll('button[data-action-id]')) {
      if (!visible(action)) continue;
      const b = action.getBoundingClientRect();
      if (Math.min(a.right, b.right) > Math.max(a.left, b.left) + 1 && Math.min(a.bottom, b.bottom) > Math.max(a.top, b.top) + 1) failures.push(`Inspection control overlaps action: ${action.getAttribute('aria-label') || action.textContent}`);
    }
  }
  // Sibling region rectangles cannot overlap even when intrinsic rows are huge.
  for (const first of regions.filter(visible)) for (const second of regions.filter(visible)) {
    if (first === second || first.parentElement !== second.parentElement) continue;
    const a = first.getBoundingClientRect(), b = second.getBoundingClientRect();
    if (Math.min(a.right, b.right) - Math.max(a.left, b.left) > 1 && Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top) > 1) failures.push(`Sibling overlap: ${first.tagName}/${second.tagName}`);
  }
  if (node.dataset.inspectorMode === 'properties') {
    for (const button of node.actionsVisualizer.querySelectorAll('button[data-action-id]')) {
      if (button.getClientRects().length && button.scrollHeight > button.clientHeight + 1) failures.push(`Clipped subject action: ${button.textContent}`);
    }
  }
  const propertyList = node.querySelector('[data-dahn-properties-list]');
  if (node.dataset.inspectorMode === 'properties' && propertyList?.getBoundingClientRect().height < 100 * (parseFloat(node.style.fontSize) / 16)) failures.push(`Properties viewport too short: ${propertyList.getBoundingClientRect().height}px`);
  return { failures, edit: node.querySelector('[data-dahn-properties] input')?.value, restored: node.dataset.restored, mode: node.dataset.inspectorMode };
}
export function reassign(presentation: string | undefined, textScale: number, compact = false) {
  const node: any = document.body.firstElementChild;
  const profile = participationProfile(16 * textScale);
  applyNodePresentation(node, { width: profile.horizontal.minimum[compact ? 0 : 2], height: profile.vertical.minimum[compact ? 0 : 2], ...(presentation ? { presentation } : {}) });
}
export function historicalOverlapDetected() {
  const owner = document.createElement('section');
  owner.style.cssText = 'position:relative;width:360px;height:280px;';
  const body = document.createElement('div'); body.style.height = '0px';
  const actions = document.createElement('div'); actions.style.cssText = 'position:absolute;top:80px;height:100px;width:360px;';
  const collection = document.createElement('div'); collection.style.cssText = 'position:absolute;top:118px;height:100px;width:360px;';
  body.append(actions); owner.append(body, collection); document.body.append(owner);
  const a = actions.getBoundingClientRect(), b = collection.getBoundingClientRect();
  const overlap = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top);
  owner.remove(); return overlap === 62;
}
export async function mountVerticalPath(height: number) {
  document.body.replaceChildren();
  const items: any[] = [];
  for (let index = 0; index < 3; index++) {
    await mount('generic', index < 2 ? 'collection' : 'properties', 1, true, 'preferred', false);
    const element: any = document.body.lastElementChild;
    items.push({ id: String(index), row: index, rowId: `row-${index}`, column: 1, element,
      ...(index ? { provenance: { parentOccurrenceId: String(index - 1), kind: 'collection-member', traversal: { label: 'Available Schemas' } } } : {}) });
  }
  const Path = await runtime.realize(ref('PathInspector.RootedNavigationVisualizer')) as CustomElementConstructor;
  const path: any = document.createElement(defineCustomElementOnce('fixture-composed-path', Path));
  let publish: any;
  path.setContext({ navigation: { subscribe(listener: any) { publish = listener; listener(items.slice(0, 1), { occurrenceId: '0', mode: 'traverse' }); return () => {}; }, restore(id: string) { publish(items, { occurrenceId: id, mode: 'restore' }); }, dispose() {} } });
  document.body.replaceChildren(path);
  Object.assign(path.style, { width: '1400px', height: `${height}px` });
  await new Promise(requestAnimationFrame); await new Promise(requestAnimationFrame);
  publish(items.slice(0, 2), { occurrenceId: '1', mode: 'traverse' });
  publish(items, { occurrenceId: '2', mode: 'traverse' });
  // Let the real viewport observer settle allocation; frontier animation never
  // changes the owner-assigned rectangles.
  await new Promise(requestAnimationFrame); await new Promise(requestAnimationFrame);
  const bounds = items.map(item => path.layoutBounds.get(item.id));
  return { modes: items.map(item => item.element.dataset.inspectorMode), bounds,
    contextViewportHeight: items[1].element.collectionViewer.getBoundingClientRect().height,
    preferred: path.getSpatialExtents().preferred,
    scale: path.view.scale,
    connectors: path.lineage.querySelectorAll('path').length,
    viewportHeight: path.viewport.clientHeight,
    contentHeight: path.view.height };
}
export async function restoreVerticalRoot() {
  const path: any = document.body.firstElementChild;
  path.navigation.restore('0');
  await new Promise(requestAnimationFrame); await new Promise(requestAnimationFrame);
  return { modes: path.occurrences.map((item: any) => item.element.dataset.inspectorMode) };
}
export async function narrowPathViewport() {
  const path: any = document.body.firstElementChild;
  const before = path.columnWidths[0];
  path.style.width = '1080px';
  await new Promise(requestAnimationFrame); await new Promise(requestAnimationFrame);
  return { before, after: path.columnWidths[0] };
}
export async function mountHorizontalPath() {
  document.body.replaceChildren();
  const items: any[] = [];
  for (let index = 0; index < 4; index++) {
    await mount('generic', 'properties', 1, true, 'preferred', false);
    const element: any = document.body.lastElementChild;
    items.push({ id: String(index), row: 0, rowId: 'row-0', column: index + 1, element,
      ...(index ? { provenance: { parentOccurrenceId: String(index - 1), kind: 'singular-relationship', traversal: { label: 'DescribedBy' } } } : {}) });
  }
  const Path = await runtime.realize(ref('PathInspector.RootedNavigationVisualizer')) as CustomElementConstructor;
  const path: any = document.createElement(defineCustomElementOnce('fixture-composed-path', Path));
  let publish: any;
  path.setContext({ navigation: { subscribe(listener: any) { publish = listener; listener(items, { occurrenceId: '3', mode: 'traverse' }); return () => {}; },
    restore(id: string) { publish(items, { occurrenceId: id, mode: 'restore' }); }, dispose() {} } });
  document.body.replaceChildren(path);
  for (const item of items) item.element.setOccurrenceRestorationHandler(() => path.navigation.restore(item.id));
  Object.assign(path.style, { width: '1080px', height: '800px' });
  await new Promise(requestAnimationFrame); await new Promise(requestAnimationFrame);
  path.stopTraversalTransition();
  path.view.position(path.layoutBounds.get('1').x, 0);
  return { inbound: [...path.boundaryCues.querySelectorAll('[data-boundary-traversal-source]')].map((cue: any) => [cue.dataset.boundaryTraversalSource, cue.dataset.boundaryTraversalTarget]),
    connectors: path.lineage.querySelectorAll('path').length };
}
export async function selectPathOccurrence(id: string) {
  const path: any = document.body.firstElementChild;
  const item = path.occurrences.find((item: any) => item.id === id);
  item.element.querySelector('[data-node-title-bar] > button').click();
  await new Promise(requestAnimationFrame); await new Promise(requestAnimationFrame);
  return path.occurrences.map((item: any) => item.element.dataset.inspectorMode);
}
export async function mountCollectionsIndex(scale = 1) {
  await mount('generic', 'properties', scale, true, 'preferred', true, 'Fixture.Subject', ['Available Schemas', 'Offers Theme', 'Owns', 'Affords Dancer']);
  applyNodePresentation(document.body.firstElementChild, { width: 800 * scale, height: 480 * scale });
  await new Promise(requestAnimationFrame); await new Promise(requestAnimationFrame);
  return collectionIndexState();
}
export function collectionIndexState() {
  const node: any = document.body.firstElementChild;
  const tabs = node.collectionTabBar;
  const visible = (element: HTMLElement) => !!element.getClientRects().length && getComputedStyle(element).visibility !== 'hidden';
  return { count: node.collectionControls.filter(visible).length,
    more: visible(tabs.querySelector('[data-overflow-more]')), pane: node.dataset.paneAttention,
    selected: node.collectionControls.find((button: HTMLElement) => button.getAttribute('aria-selected') === 'true')?.textContent,
    tabWidth: tabs.getBoundingClientRect().width,
    rowWidth: tabs.querySelector('[data-overflow-row]').getBoundingClientRect().width };
}
export async function descendFromRestoredMiddle() {
  const path: any = document.body.firstElementChild;
  const items = [...path.occurrences];
  const source = items.find(item => item.id === '1');
  path.navigation.restore(source.id);
  source.element.setNodeOwnedCollection(source.element.querySelector('nav[aria-label="Non-table Collection"]'));
  const retainedWidths = items.filter(item => item.id !== source.id).map(item => [item.id, path.layoutBounds.get(item.id).width]);
  // Materialize a real destination without replacing the existing path or nodes.
  await mount('generic', 'properties', 1, true, 'minimum', false);
  const element = document.body.lastElementChild;
  const destination = { id: 'down', row: 1, rowId: 'row-down', column: source.column, element,
    provenance: { parentOccurrenceId: source.id, kind: 'collection-member', traversal: { label: 'Instance Relationships' } } };
  path.renderPath([...items, destination], { occurrenceId: 'down', mode: 'traverse' });
  await new Promise(requestAnimationFrame); await new Promise(requestAnimationFrame);
  path.stopTraversalTransition();
  return { modes: path.occurrences.map((item: any) => [item.id, item.element.dataset.inspectorMode]),
    widths: retainedWidths.map(([id, width]) => [width, path.layoutBounds.get(id).width]) };
}
export async function mountCompactAspect(kind: string, width: number, height: number, scale: number) {
  await mount(kind, 'compact', scale, true, 'minimum', true, 'MetaHolonType.MetaTypeDescriptor');
  const node: any = document.body.firstElementChild;
  applyNodePresentation(node, { width: width * scale, height: height * scale, presentation: 'compact' });
  await new Promise(requestAnimationFrame); await new Promise(requestAnimationFrame);
  const title = node.titleBar.titleControl, identity = title.querySelector('[data-node-title-bar] > button'), close = node.titleBar.actionsControl.querySelector('[data-action-id="CloseOccurrence.VisualizerOperator"]');
  return { node: node.getBoundingClientRect().toJSON(), header: node.titleBar.element.getBoundingClientRect().toJSON(),
    identity: identity.getBoundingClientRect().toJSON(), close: close.getBoundingClientRect().toJSON(),
    stacked: node.titleBar.element.dataset.titleLayout === 'stacked',
    wrapped: getComputedStyle(identity).whiteSpace === 'normal', textFits: identity.scrollHeight <= identity.clientHeight + 1,
    failures: measure().failures };
}
(window as any).allocationFixture = { mountCollectionsIndex, collectionIndexState, descendFromRestoredMiddle, mountCompactAspect, mount, measure, reassign, historicalOverlapDetected, mountVerticalPath, restoreVerticalRoot, narrowPathViewport, mountHorizontalPath, selectPathOccurrence };
