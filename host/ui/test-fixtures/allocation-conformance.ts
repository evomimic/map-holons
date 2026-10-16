import { composeNodeActions } from '../src/dahn/runtime/node-actions';
import { bindVisualizerInformationControl } from '../src/dahn/runtime/visualizer-information-control';
import { composeNodeTitle } from '../src/dahn/runtime/title-composition';
import { MaterializedVisualizerRuntime } from '../src/dahn/runtime/materialized-visualizer-runtime';
import { MaterializedVisualizerCache } from '../src/dahn/runtime/materialized-visualizer-cache';
import { defineCustomElementOnce } from '../src/dahn/visualizers/define-custom-element-once';
import { applyNodePresentation, nodeCapabilities, participationProfile } from '../../conductora/visualizer-src/node-presentation.js';

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
export async function mount(kind: string, horizontal: string, vertical: string, textScale = 1, activeCollection = true) {
  document.body.replaceChildren();
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
  const actions = await composeNodeActions(transaction, runtime, owner, { target: { reference: subject }, actions: [], theme: { reference: ref('fixture-theme') } } as any);
  const collection = document.createElement('nav'); collection.setAttribute('aria-label', 'Non-table Collection');
  collection.style.cssText = 'display:flex;flex-direction:column;min-height:0;min-width:0;overflow:auto;flex:1;';
  for (let index = 0; index < 30; index++) {
    const button = document.createElement('button'); button.textContent = `Open Collection member ${index}`;
    button.style.cssText = 'flex:0 0 auto;font:inherit;'; collection.append(button);
  }
  const context: any = { title: 'Holon: A long readable subject identity', holonKey: 'Fixture.Subject', target: { reference: subject }, actions: [],
    theme: { reference: ref('fixture-theme') }, canvas: {}, nodeAffordances: { singularRelationships: ['Navigate singular relationship', 'Owned by', 'Described by', 'Version of', 'Source', 'Parent'].map(label => ({ label, relationship: {} })), collections: [] }, relationshipDiscovery: { population: () => ({ state: 'populated', count: 1 }), subscribe: (listener: () => void) => { listener(); return () => {}; } }, activateRelationship: () => {},
    childVisualizers: new Map([['properties', property], ...(activeCollection ? [['collections', collection]] : []), ['actions', actions]]) };
  const title = await composeNodeTitle(transaction, runtime, owner, context);
  context.childVisualizers.set('titleBar', title);
  node.setContext(context);
  node.setOccurrenceRestorationHandler(() => { node.dataset.restored = 'true'; });
  node.setOccurrenceExplorationHandler(() => {}); node.setOccurrenceClosureHandler(() => {});
  node.setVisualizerInformationHandler(() => {}, 'Fixture Node');
  node.style.fontSize = `${16 * textScale}px`;
  const profile = participationProfile(16 * textScale);
  const index = (state: string) => state === 'expanded' ? 2 : state === 'partial' ? 1 : 0;
  const grant = { width: profile.horizontal.minimum[index(horizontal)], height: profile.vertical.minimum[index(vertical)], requiredCapabilities: nodeCapabilities(horizontal, vertical) };
  document.body.append(node);
  await runtime.admitOffered(node, owner);
  applyNodePresentation(node, grant);
  await new Promise(requestAnimationFrame); await new Promise(requestAnimationFrame);
  return { grant, bounds: node.getBoundingClientRect().toJSON() };
}
export function measure() {
  const node: any = document.body.firstElementChild;
  const visible = (element: HTMLElement) => !!element.getClientRects().length && getComputedStyle(element).visibility !== 'hidden';
  const failures: string[] = [];
  // Each binding supplies its immediate owned regions; conformance does not use
  // private region names to establish Operator eligibility.
  const regions: HTMLElement[] = node.dataset.dahnHolonInspector ? [node.titleBar.element, node.body, node.collectionRegion, node.propertyViewer, node.actionBar, node.singleValueRail, node.collectionViewer]
    : [node.heading, node.body, node.content, node.properties, node.collections, node.rail];
  for (const region of regions.filter(visible)) {
    const parent = region.parentElement!.getBoundingClientRect(), child = region.getBoundingClientRect();
    if (child.left < parent.left - 1 || child.top < parent.top - 1 || child.right > parent.right + 1 || child.bottom > parent.bottom + 1) failures.push(`${region.getAttribute('aria-label') || region.tagName} exceeds owner: ${JSON.stringify(child.toJSON())} / ${JSON.stringify(parent.toJSON())}`);
  }
  // Sibling region rectangles cannot overlap even when intrinsic rows are huge.
  for (const first of regions.filter(visible)) for (const second of regions.filter(visible)) {
    if (first === second || first.parentElement !== second.parentElement) continue;
    const a = first.getBoundingClientRect(), b = second.getBoundingClientRect();
    if (Math.min(a.right, b.right) - Math.max(a.left, b.left) > 1 && Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top) > 1) failures.push(`Sibling overlap: ${first.tagName}/${second.tagName}`);
  }
  const propertyList = node.querySelector('[data-dahn-properties-list]');
  if (node.getNodeInspectorAllocation().requiredCapabilities.includes('inspect') && propertyList?.getBoundingClientRect().height < 100 * (parseFloat(node.style.fontSize) / 16)) failures.push(`Properties viewport too short: ${propertyList.getBoundingClientRect().height}px`);
  return { failures, edit: node.querySelector('[data-dahn-properties] input')?.value, restored: node.dataset.restored, capabilities: node.getNodeInspectorAllocation().requiredCapabilities };
}
export function reassign(horizontal: string, vertical: string, textScale: number) {
  const node: any = document.body.firstElementChild;
  const profile = participationProfile(16 * textScale);
  const index = (state: string) => ['compact', 'partial', 'expanded'].indexOf(state);
  applyNodePresentation(node, { width: profile.horizontal.minimum[index(horizontal)], height: profile.vertical.minimum[index(vertical)], requiredCapabilities: nodeCapabilities(horizontal, vertical) });
}
// Preserve the observed failure shape without shipping its retired renderer:
// intrinsic action rows intersect a Collection after the body collapses to zero.
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
(window as any).allocationFixture = { mount, measure, reassign, historicalOverlapDetected };
