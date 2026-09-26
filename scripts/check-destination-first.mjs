// Real navigation/activation coordination and shipped artifacts with controlled semantic fixtures.
// Run with MAP_PLAYWRIGHT_MODULE / MAP_BROWSER_EXECUTABLE as in check-navigation-compression.mjs.
import { readFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';
const { chromium } = await import(process.env.MAP_PLAYWRIGHT_MODULE ?? 'playwright');
const root = fileURLToPath(new URL('..', import.meta.url));
const dataUrl = source => `data:text/javascript;base64,${Buffer.from(source).toString('base64')}`;
const sources = {};
for (const name of ['holon-inspector', 'path-inspector', 'table-collection']) {
  sources[name] = dataUrl(await readFile(`${root}/host/conductora/resources/dahn-visualizers/${name}.js`, 'utf8'));
}
const bundle = await build({ stdin: { contents: `
export { PathNavigator } from './host/ui/src/dahn/runtime/path-navigator';
export { NodeCollectionActivation } from './host/ui/src/dahn/runtime/collection-activation';
export { NodeRelationshipDiscovery } from './host/ui/src/dahn/runtime/relationship-discovery';`, resolveDir: root }, bundle: true, format: 'esm', platform: 'browser', write: false });
sources.runtime = dataUrl(bundle.outputFiles[0].text);
const theme = await readFile(`${root}/host/ui/src/launcher-theme.generated.css`, 'utf8');
const brave = '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser';
const browser = await chromium.launch({ executablePath: process.env.MAP_BROWSER_EXECUTABLE ?? (existsSync(brave) ? brave : undefined), headless: true });
const report = [];
try {
  for (const reducedMotion of ['no-preference', 'reduce']) {
    const page = await browser.newPage({ viewport: { width: 1100, height: 850 }, reducedMotion });
    const errors = []; page.on('pageerror', error => errors.push(error.message));
    await page.route('https://map-fixture.test/', route => route.fulfill({ contentType: 'text/html', body: '<html></html>' }));
    await page.goto('https://map-fixture.test/');
    await page.setContent(`<style>${theme} *{box-sizing:border-box} body{margin:0;font-family:system-ui;background:var(--dahn-canvas-surface-background);color:var(--dahn-canvas-text-color)} main{height:810px;display:flex;padding:20px}</style><main></main>`);
    await page.evaluate(async sources => {
      const { PathNavigator, NodeCollectionActivation, NodeRelationshipDiscovery } = await import(sources.runtime);
      customElements.define('check-destination-node', (await import(sources['holon-inspector'])).default);
      customElements.define('check-destination-path', (await import(sources['path-inspector'])).default);
      const Table = (await import(sources['table-collection'])).default;
      const assert = (value, message) => { if (!value) throw Error(message); };
      const fixture = window.fixture = { assert, samples: [], paths: [], destination: undefined, fail: false };
      const property = { propertyName: async () => 'Key', displayName: async () => 'Key', valueKind: async () => 'StringValue', isArray: async () => false };
      const collection = members => ({ length: members.length, elementType: { hasInstanceKey: async () => false, instanceProperties: async () => [property] }, [Symbol.iterator]: () => members[Symbol.iterator]() });
      const reference = label => ({ holonId: async () => ({ Local: label }), key: async () => label,
        holonDescriptor: async () => ({ hasInstanceKey: async () => false }), propertyValue: async () => ({ StringValue: label }),
        relatedHolons: async name => {
          if (fixture.membershipGate) await fixture.membershipGate;
          return collection(fixture.emptyTarget ? [] : name === 'First' ? [a] : name === 'Second' ? [b] : name === 'Empty' ? [] : [a, b]);
        },
        describedRelatedHolons: async name => {
          if (fixture.collectionGate) await fixture.collectionGate;
          return collection(name === 'Empty' ? [] : [a, b]);
        } });
      const a = reference('Member A'), b = reference('Member B'), subject = reference('Root');
      const tabs = ['Owns', 'Offers', 'Empty'].map(label => ({ kind: 'relationship', label, relationship: { direction: 'declared', descriptor: { relationshipName: async () => label, isOrdered: async () => false } } }));
      const singular = ['First', 'Second'].map(label => ({ label, relationship: { direction: 'declared', descriptor: { relationshipName: async () => label } } }));
      const transaction = { getSavedHolonByBaseKey: async () => ({}), selectCollectionVisualizer: async () => ({ selected: {} }),
        selectVisualizer: async () => { if (fixture.selectionGate) await fixture.selectionGate; if (fixture.fail) throw Error('Selection unavailable'); return { selected: {} }; } };
      const materialized = { realize: async () => Table };
      const makeNode = (ref, title) => {
        const element = document.createElement('check-destination-node');
        const properties = document.createElement('input'); properties.value = 'Preserved draft';
        const discovery = new NodeRelationshipDiscovery(transaction, ref, [...tabs, ...singular]);
        for (const relation of singular) discovery.record(relation, 1);
        for (const tab of tabs) discovery.record(tab, tab.label === 'Empty' ? 0 : 2);
        const activation = new NodeCollectionActivation(transaction, ref, {}, materialized, discovery);
        element.setContext({ title, holonKey: title, relationshipDiscovery: discovery, collectionActivation: activation,
          activateRelationship: affordance => fixture.navigation.traverseRelationship({ source: element, affordance }),
          nodeAffordances: { collections: tabs, singularRelationships: singular }, childVisualizers: new Map([['properties', properties]]) });
        return { element, collectionActivation: activation, singularRelationships: singular, relationshipDiscovery: discovery };
      };
      const rootNode = makeNode(subject, 'Root');
      const navigation = new PathNavigator(transaction, {}, rootNode, subject, {}, {}, async ref => makeNode(ref, await ref.key()));
      navigation.subscribe((paths, focus, destination) => Object.assign(fixture, { paths, focus, destination }));
      const path = document.createElement('check-destination-path');
      path.setContext({ navigation, onInspectHolon: intent => navigation.inspect(intent) });
      document.querySelector('main').append(path);
      Object.assign(fixture, { path, navigation, rootNode, a, b, tabs });
      // Sample real browser frames: cached results must leave a visible pending
      // layout for a rendering opportunity before content arrives.
      const sample = () => {
        const region = path.querySelector('[data-path-destination]');
        const viewer = rootNode.element.collectionViewer;
        const pending = region ?? (viewer.dataset.collectionState === 'loading' ? viewer : undefined);
        if (pending) {
          const bounds = pending.getBoundingClientRect();
          const text = pending.textContent;
          fixture.samples.push({ text, width: bounds.width, height: bounds.height, frameTime: performance.now() });
        }
        fixture.frame = requestAnimationFrame(sample);
      };
      fixture.frame = requestAnimationFrame(sample);
    }, sources);
    const rootNode = page.locator('check-destination-node').first();
    await rootNode.getByRole('tab', { name: 'Owns (2)', exact: true }).click();
    await rootNode.locator('table').waitFor({ timeout: 5000 }).catch(async error => { console.error(await page.locator('body').innerText()); throw error; });
    await page.evaluate(() => {
      fixture.assert(fixture.samples.some(sample => sample.text.includes('Opening Owns') && sample.height > 0), `Immediate collection skipped pending paint: ${JSON.stringify(fixture.samples)}`);
      fixture.viewer = fixture.rootNode.element.collectionViewer;
      fixture.bounds = fixture.viewer.getBoundingClientRect().toJSON();
      fixture.collectionGate = new Promise(resolve => { fixture.finishCollection = resolve; });
    });
    await rootNode.getByRole('tab', { name: 'Offers (2)', exact: true }).click();
    await rootNode.getByText('Opening Offers…', { exact: true }).waitFor();
    await page.evaluate(() => {
      fixture.assert(fixture.rootNode.element.collectionViewer === fixture.viewer, 'Collection region replaced');
      const bounds = fixture.viewer.getBoundingClientRect();
      fixture.assert(Math.abs(bounds.height - fixture.bounds.height) < 2, 'Collection collapsed while switching');
      fixture.collectionGate = undefined; fixture.finishCollection();
    });
    await rootNode.locator('table[aria-label="Offers"]').waitFor();
    await rootNode.getByRole('checkbox', { name: 'Show Empty Relationships' }).check();
    await rootNode.getByRole('tab', { name: 'Empty (0)', exact: true }).click();
    await page.evaluate(() => {
      fixture.assert(fixture.viewer.querySelector('table')?.getAttribute('aria-label') === 'Offers', 'Empty inspection replaced collection');
      fixture.assert(fixture.paths.length === 1, 'Empty inspection traversed');
    });
    await rootNode.locator('tbody tr').first().dblclick();
    await page.waitForFunction(() => fixture.paths.length === 2);
    await page.evaluate(() => {
      fixture.assert(fixture.samples.some(sample => sample.text.includes('Opening selected holon') && sample.height > 0), 'Immediate member skipped pending paint');
      fixture.leaf = fixture.paths[1]; fixture.leafRegion = fixture.leaf.element.parentElement;
      fixture.leaf.element.querySelector('input:not([type=checkbox])').value = 'Retained draft';
      fixture.fail = true;
    });
    await rootNode.locator('tbody tr').nth(1).dblclick();
    const pending = page.locator('[data-path-destination]');
    await pending.getByRole('button', { name: 'Retry opening holon' }).waitFor();
    await page.evaluate(() => {
      fixture.assert(fixture.leaf.element.isConnected, 'Old leaf was detached');
      fixture.assert(fixture.leafRegion.inert, 'Occluded leaf remains interactive');
      fixture.assert(!fixture.path.querySelector(`[data-lineage-child="${fixture.destination.id}"]`), 'Pending destination published a connector');
    });
    if (reducedMotion === 'no-preference' && process.env.MAP_DESTINATION_SCREENSHOT) await page.screenshot({ path: process.env.MAP_DESTINATION_SCREENSHOT });
    await pending.getByRole('button', { name: 'Cancel', exact: true }).focus();
    await page.keyboard.press('Enter');
    await page.evaluate(() => {
      fixture.assert(!fixture.destination && !fixture.leafRegion.inert, 'Cancel did not restore leaf');
      fixture.assert(document.activeElement === fixture.leafRegion, 'Keyboard focus not recovered');
      fixture.assert(fixture.leaf.element.querySelector('input:not([type=checkbox])').value === 'Retained draft', 'Draft state lost');
      fixture.fail = false;
      fixture.selectionGate = new Promise(resolve => { fixture.finishSelection = resolve; });
    });
    await rootNode.locator('tbody tr').nth(1).dblclick();
    await page.waitForFunction(() => fixture.destination?.pending);
    await page.evaluate(() => {
      fixture.pendingId = fixture.destination.id;
      fixture.pendingRegion = fixture.path.querySelector('[data-path-destination]');
      fixture.selectionGate = undefined; fixture.finishSelection();
    });
    await page.waitForFunction(() => !fixture.destination && fixture.paths[1].subject === fixture.b);
    await page.evaluate(() => {
      fixture.assert(fixture.paths[1].id === fixture.pendingId && fixture.paths[1].element.parentElement === fixture.pendingRegion, 'Content moved away from reserved region');
      fixture.navigation.restore(fixture.paths[0].id);
    });
    await page.evaluate(() => {
      fixture.membershipGate = new Promise(resolve => { fixture.finishMembership = resolve; });
      fixture.selectionGate = new Promise(resolve => { fixture.finishSelection = resolve; });
      fixture.columns = fixture.path.viewport.style.gridTemplateColumns;
    });
    await rootNode.getByRole('button', { name: 'First (1)', exact: true }).click();
    await page.evaluate(() => {
      fixture.assert(!fixture.destination, 'Horizontal check allocated before existence');
      fixture.assert(fixture.path.viewport.style.gridTemplateColumns === fixture.columns, 'Check compressed the source');
      fixture.membershipGate = undefined; fixture.finishMembership();
    });
    await page.waitForFunction(() => fixture.destination?.message === 'Opening First…');
    await page.locator('[data-path-destination]').waitFor({ state: 'visible' });
    await page.evaluate(() => {
      const source = fixture.rootNode.element.parentElement;
      const destination = fixture.path.querySelector('[data-path-destination]');
      const sourceBounds = source.getBoundingClientRect(), bounds = destination.getBoundingClientRect();
      fixture.assert(source.dataset.columnAllocation === 'partial', 'Source did not partially compress');
      fixture.assert(destination.dataset.columnAllocation === 'expanded' && bounds.width > sourceBounds.width * 2, 'Pending destination is squeezed');
      fixture.assert(Math.abs(sourceBounds.y - bounds.y) < 1, 'Horizontal destination changed row');
      fixture.assert(!fixture.path.querySelector(`[data-lineage-child="${fixture.destination.id}"]`), 'Pending horizontal connector');
      fixture.horizontalRegion = destination; fixture.horizontalId = fixture.destination.id;
      fixture.horizontalBounds = bounds.toJSON();
    });
    if (reducedMotion === 'no-preference' && process.env.MAP_HORIZONTAL_SCREENSHOT) await page.screenshot({ path: process.env.MAP_HORIZONTAL_SCREENSHOT });
    await page.evaluate(() => { fixture.selectionGate = undefined; fixture.finishSelection(); });
    await page.waitForFunction(() => !fixture.destination && fixture.paths.some(item => item.id === fixture.horizontalId));
    await page.evaluate(() => {
      fixture.horizontal = fixture.paths.find(item => item.id === fixture.horizontalId);
      fixture.assert(fixture.horizontal.element.parentElement === fixture.horizontalRegion, 'Horizontal content changed regions');
      const bounds = fixture.horizontalRegion.getBoundingClientRect();
      fixture.assert(Math.abs(bounds.width - fixture.horizontalBounds.width) < 1 && Math.abs(bounds.height - fixture.horizontalBounds.height) < 1, 'Horizontal fill changed geometry');
      fixture.assert(fixture.samples.some(sample => sample.text.includes('Opening First')), 'Horizontal pending skipped frames');
    });
    // Recursive immediate results must paint too; this also makes the old branch retained.
    await page.evaluate(() => fixture.horizontal.element.querySelector('[data-singular-relationship]').click());
    await page.waitForFunction(() => !fixture.destination && fixture.paths.filter(item => item.provenance?.kind === 'singular-relationship').length === 2);
    await page.evaluate(() => {
      fixture.retained = fixture.paths.map(item => [item.id, item.rowId, item.column]);
      fixture.navigation.restore(fixture.paths[0].id);
      fixture.fail = true;
      fixture.rootNode.element.querySelectorAll('[data-singular-relationship]')[1].click();
    });
    await page.locator('[data-path-destination]').getByRole('button', { name: 'Retry opening holon' }).waitFor();
    await page.evaluate(() => {
      fixture.assert(fixture.paths.find(item => item.id === fixture.horizontalId).row === 1, 'Retained horizontal branch not displaced');
      fixture.assert(fixture.paths.every(item => item.element.isConnected), 'Retained mixed paths detached');
      fixture.destination.cancel();
      fixture.assert(JSON.stringify(fixture.paths.map(item => [item.id, item.rowId, item.column])) === JSON.stringify(fixture.retained), 'Cancel did not restore retained grid');
      fixture.emptyTarget = true; fixture.fail = false;
      fixture.columns = fixture.path.viewport.style.cssText;
      fixture.sourceBounds = fixture.rootNode.element.getBoundingClientRect().toJSON();
      fixture.rootNode.element.querySelectorAll('[data-singular-relationship]')[1].click();
    });
    await page.waitForFunction(() => !fixture.paths[0].pending);
    await page.evaluate(() => {
      fixture.assert(!fixture.destination, 'Zero allocated a destination');
      fixture.assert(fixture.path.viewport.style.cssText === fixture.columns, 'Zero changed grid allocation');
      const bounds = fixture.rootNode.element.getBoundingClientRect();
      fixture.assert(Math.abs(bounds.width - fixture.sourceBounds.width) < 1 && Math.abs(bounds.height - fixture.sourceBounds.height) < 1, 'Zero changed source extent');
      cancelAnimationFrame(fixture.frame);
    });
    if (errors.length) throw Error(errors.join('\n'));
    report.push({ reducedMotion, pendingPaint: true, regionReuse: true, emptyInspectionPreserved: true, cancelRestoredStateAndFocus: true, horizontalPartialCompression: true, horizontalRegionReuse: true, recursiveRetention: true });
    await page.close();
  }
  console.log(JSON.stringify(report, null, 2));
} finally { await browser.close(); }
