// Shipped Node artifact plus real discovery coordination; semantic reads are controlled fixtures.
// Use MAP_PLAYWRIGHT_MODULE / MAP_BROWSER_EXECUTABLE as in check-navigation-compression.mjs.
import { readFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';
const { chromium } = await import(process.env.MAP_PLAYWRIGHT_MODULE ?? 'playwright');
const root = fileURLToPath(new URL('..', import.meta.url));
const theme = await readFile(`${root}/host/ui/src/launcher-theme.generated.css`, 'utf8');
const nodeSource = await readFile(`${root}/host/conductora/resources/dahn-visualizers/holon-inspector.js`, 'utf8');
const bundle = await build({ stdin: { contents: `export { NodeRelationshipDiscovery } from './host/ui/src/dahn/runtime/relationship-discovery';`, resolveDir: root }, bundle: true, format: 'esm', platform: 'browser', write: false });
const dataUrl = source => `data:text/javascript;base64,${Buffer.from(source).toString('base64')}`;
const brave = '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser';
const browser = await chromium.launch({ executablePath: process.env.MAP_BROWSER_EXECUTABLE ?? (existsSync(brave) ? brave : undefined), headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1000, height: 780 } });
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.setContent(`<style>${theme} *{box-sizing:border-box} body{font-family:system-ui;background:var(--dahn-canvas-surface-background);color:var(--dahn-canvas-text-color)} main{height:680px;width:900px;display:flex;padding:20px}</style><main></main>`);
  await page.evaluate(async ({ node, discovery }) => {
    const Node = (await import(node)).default;
    const { NodeRelationshipDiscovery } = await import(discovery);
    customElements.define('check-discovery-node', Node);
    const relation = (label, plural = false) => ({ ...(plural ? { kind: 'relationship' } : {}), label, relationship: { direction: 'declared', descriptor: { relationshipName: async () => label } } });
    const parent = relation('Parent'), friends = relation('Friends', true), orders = relation('Orders', true), failed = relation('Unavailable', true);
    const pending = new Map();
    const owner = { relatedHolons: name => new Promise((resolve, reject) => pending.set(name, { resolve, reject })) };
    const population = new NodeRelationshipDiscovery({}, owner, [parent, friends, orders, failed]);
    const element = document.createElement('check-discovery-node');
    const properties = document.createElement('input'); properties.value = 'Preserved local state';
    let activations = 0;
    element.setContext({ title: 'Person: Example', relationshipDiscovery: population,
      activateRelationship: () => ++activations,
      collectionActivation: { activate: () => ++activations, dispose: () => population.dispose() },
      nodeAffordances: { singularRelationships: [parent], collections: [friends, orders, failed, { kind: 'property', label: 'Array property' }] },
      childVisualizers: new Map([['properties', properties]]) });
    document.querySelector('main').append(element); element.setSpatialBudget({ width: 860, height: 640 });
    population.startAfterDisplay(element);
    window.fixture = { element, population, properties, pending, activations: () => activations };
  }, { node: dataUrl(nodeSource), discovery: dataUrl(bundle.outputFiles[0].text) });
  await page.waitForFunction(() => fixture.pending.size === 2);
  await page.evaluate(() => {
    if (!fixture.properties.isConnected || fixture.properties.value !== 'Preserved local state') throw Error('Initial content blocked');
    if ([...fixture.element.relationshipControls.values()].some(({ button }) => button.getBoundingClientRect().height)) throw Error('Unverified browse affordance exposed');
    fixture.pending.get('Friends').resolve({ length: 0 });
  });
  await page.waitForFunction(() => fixture.pending.has('Orders'));
  await page.evaluate(() => fixture.pending.get('Orders').resolve({ length: 8 }));
  await page.waitForFunction(() => fixture.pending.has('Unavailable'));
  await page.evaluate(() => fixture.pending.get('Unavailable').reject(Error('Offline fixture')));
  await page.getByText('Orders (8)', { exact: true }).waitFor({ state: 'visible' });
  await page.getByRole('checkbox', { name: 'Show Empty Relationships' }).check();
  await page.getByRole('tab', { name: 'Friends (0)' }).click();
  await page.getByText('Friends: No targets.', { exact: true }).waitFor();
  if (await page.evaluate(() => fixture.activations()) !== 0) throw Error('Empty activation navigated');
  await page.evaluate(() => fixture.pending.get('Parent').resolve({ length: 1 }));
  await page.getByRole('button', { name: 'Parent (1)', exact: true }).waitFor({ state: 'visible' });
  await page.getByRole('button', { name: 'Retry Unavailable' }).click();
  await page.evaluate(() => fixture.pending.get('Unavailable').resolve({ length: 2 }));
  await page.getByRole('tab', { name: 'Unavailable (2)' }).waitFor({ state: 'visible' });
  await page.getByRole('checkbox', { name: 'Show Empty Relationships' }).uncheck();
  const report = await page.evaluate(() => {
    if (fixture.properties.value !== 'Preserved local state') throw Error('Local state replaced');
    const labels = [...fixture.element.collectionControls].map(button => button.textContent);
    if (labels.join('|') !== 'Friends (0)|Orders (8)|Unavailable (2)') throw Error('Arrival reordered descriptors');
    return { labels, initialReadOverlap: 2, localStatePreserved: true, emptyActivationNavigated: false };
  });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.getByRole('checkbox', { name: 'Show Empty Relationships' }).check();
  await page.getByRole('tab', { name: 'Friends (0)' }).waitFor({ state: 'visible' });
  if (process.env.MAP_DISCOVERY_SCREENSHOT) await page.screenshot({ path: process.env.MAP_DISCOVERY_SCREENSHOT });
  if (errors.length) throw Error(errors.join('\n'));
  console.log(JSON.stringify(report, null, 2));
} finally {
  await browser.close();
}
