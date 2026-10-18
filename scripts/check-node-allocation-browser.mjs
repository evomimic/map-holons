/** Real-browser containment and keyboard conformance without changing dependencies.
 * Set MAP_PLAYWRIGHT_MODULE to an installed Playwright module when it is not local.
 */
import { build } from 'esbuild';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { createServer } from 'node:http';
const { chromium } = await import(process.env.MAP_PLAYWRIGHT_MODULE || 'playwright');
const root = resolve(import.meta.dirname, '..');
const directory = await mkdtemp(join(tmpdir(), 'map-allocation-'));
await build({ absWorkingDir: root, entryPoints: ['host/ui/test-fixtures/allocation-conformance.ts'], outfile: join(directory, 'fixture.js'), bundle: true, format: 'esm', target: 'es2022' });
const server = createServer(async (request, response) => {
  try {
    const url = new URL(request.url, 'http://localhost');
    let path;
    if (url.pathname === '/') { response.setHeader('content-type', 'text/html'); response.end('<!doctype html><style>body{margin:0;font:16px/1.25 system-ui;--dahn-slot-border-width:1px;--dahn-slot-border-style:solid;--dahn-slot-border-color:#567;--dahn-canvas-font-size:1em;--dahn-slot-padding:4px;--dahn-control-gap:4px;--dahn-canvas-gap:8px;--dahn-action-padding-block:6px;--dahn-action-padding-inline:8px;--dahn-inspector-rail-width:14rem;--dahn-action-text-color:#222;--dahn-action-surface-background:#ddd;--dahn-canvas-text-color:#222;--dahn-panel-surface-background:#eee}</style><script type="module" src="/fixture.js"></script>'); return; }
    if (url.pathname === '/fixture.js') path = join(directory, 'fixture.js');
    else if (url.pathname === '/schema.json') path = join(root, 'generated/json-imports/dahn/schema.json');
    else if (/^\/resources\/[a-z-]+\.js$/.test(url.pathname)) path = join(root, 'host/conductora/resources/dahn-visualizers', url.pathname.split('/').at(-1));
    else { response.writeHead(404).end(); return; }
    response.setHeader('content-type', path.endsWith('.js') ? 'text/javascript' : 'application/json'); response.end(await readFile(path));
  } catch (error) { response.writeHead(500).end(String(error)); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
let browser;
try {
  browser = await chromium.launch({ channel: 'chrome', headless: true });
  const page = await browser.newPage({ viewport: { width: 1600, height: 1200 } });
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  await page.waitForFunction(() => !!window.allocationFixture);
  if (!await page.evaluate(() => window.allocationFixture.historicalOverlapDetected())) throw new Error('Historical overlap regression oracle failed');
  let cases = 0;
  for (const kind of ['generic', 'loader']) for (const scale of [1, 1.25, 2]) for (const mode of ['properties', 'collection', 'relationship', 'compact']) for (const budget of ['minimum', 'preferred']) {
    await page.evaluate(args => window.allocationFixture.mount(...args), [kind, mode, scale, true, budget]);
    const result = await page.evaluate(() => window.allocationFixture.measure());
    const context = `${kind} ${mode} ${budget} @${scale}`;
    if (result.failures.length) throw new Error(`${context}: ${result.failures.join('; ')}`);
    if (result.mode !== mode) throw new Error(`${context}: wrong presentation ${result.mode}`);
    const bars = await page.locator('[data-dahn-node-actions]').count();
    if (bars !== 2) throw new Error(`${context}: expected independently selected subject and control ActionBars`);
    if (mode === 'properties') {
      const scopes = await page.evaluate(() => {
        const node = document.body.firstElementChild;
        const properties = node.propertyViewer || node.properties;
        return { header: node.propertiesHeading.getBoundingClientRect().toJSON(), pane: properties.getBoundingClientRect().toJSON() };
      });
      if (Math.abs(scopes.header.left - scopes.pane.left) > 1 || Math.abs(scopes.header.right - scopes.pane.right) > 1) throw new Error(`${context}: Properties header misrepresents its pane scope: ${JSON.stringify(scopes)}`);
    }
    if (mode === 'properties') {
      const pane = 'properties';
      const before = await page.evaluate(() => {
        const node = document.body.firstElementChild;
        window.retainedPaneChild = node.querySelector('[data-dahn-properties] input');
        window.retainedCollection = node.querySelector('nav[aria-label="Non-table Collection"]');
        node.setOccurrenceAttentionHandler(() => { throw new Error('Local pane escaped to Path attention'); });
        return node.getBoundingClientRect().toJSON();
      });
      const control = page.locator(`[data-pane-attention-control="${pane}"]`);
      await control.click();
      const expanded = await page.evaluate(pane => {
        const node = document.body.firstElementChild, title = node.titleBar.element.getBoundingClientRect();
        const region = pane === 'properties' ? node.body : (node.collectionRegion || node.collections);
        return { outer: node.getBoundingClientRect().toJSON(), region: region.getBoundingClientRect().toJSON(), titleBottom: title.bottom,
          pane: node.dataset.paneAttention, mode: node.dataset.inspectorMode, failures: window.allocationFixture.measure().failures };
      }, pane);
      if (expanded.pane !== pane || expanded.mode !== mode || JSON.stringify(expanded.outer) !== JSON.stringify(before)) throw new Error(`${context}: local pane changed Node allocation or presentation`);
      if (Math.abs(expanded.region.top - expanded.titleBottom) > 1 || Math.abs(expanded.region.bottom - before.bottom) > 1) throw new Error(`${context}: pane does not fill space below Title Bar`);
      if (expanded.failures.length) throw new Error(`${context}: expanded pane containment: ${expanded.failures.join('; ')}`);
      await control.focus(); await page.keyboard.press('Enter');
      const restored = await page.evaluate(() => { const node = document.body.firstElementChild; return { pane: node.dataset.paneAttention,
        retained: node.contains(window.retainedPaneChild) && node.contains(window.retainedCollection) }; });
      if (restored.pane || !restored.retained) throw new Error(`${context}: local Restore lost selected children`);
    }
    if (mode === 'relationship' || mode === 'compact') {
      const identity = page.locator('[data-node-title-bar] > button'); await identity.focus(); await page.keyboard.press('Enter');
      if (!(await page.evaluate(() => window.allocationFixture.measure())).restored) throw new Error(`${context}: keyboard Restore failed`);
    }
    if (mode === 'properties' || mode === 'relationship') {
      const singular = page.getByRole('button', { name: /^Navigate singular relationship/ });
      if (!await singular.isVisible()) await page.getByRole('button', { name: 'More relationships', exact: true }).click();
      await singular.focus(); await singular.evaluate(element => element.scrollIntoView({ block: 'nearest' }));
      if (!await singular.evaluate(element => { const r = element.getBoundingClientRect(); return document.activeElement === element && element.contains(document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2)); })) throw new Error(`${context}: relationship control is obscured`);
    }
    if (mode === 'collection') {
      if (await page.locator('[data-pane-attention-control=collection]').count()) throw new Error(`${context}: redundant Collection Expand control`);
      if (!await page.getByRole('button', { name: 'Properties, Actions & Relationships', exact: true }).isVisible()) throw new Error(`${context}: Collection return is unavailable`);
      const member = page.getByRole('button', { name: 'Open Collection member 29', exact: true });
      await member.focus(); await member.evaluate(element => element.scrollIntoView({ block: 'nearest' }));
      if (!await member.evaluate(element => { const r = element.getBoundingClientRect(); return document.activeElement === element && element.contains(document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2)); })) throw new Error(`${context}: collection navigation is obscured`);
      await page.getByRole('button', { name: 'Properties, Actions & Relationships', exact: true }).click();
      const local = await page.evaluate(() => window.allocationFixture.measure());
      if (local.mode !== 'properties' || local.edit !== result.edit) throw new Error(`${context}: local Properties return lost state`);
      // Re-enter Collection using the same selected child; no selection or materialization.
      await page.evaluate(() => { const node = document.body.firstElementChild; const child = node.querySelector('nav[aria-label="Non-table Collection"]'); node.setNodeOwnedCollection(child); });
    }
    const remembered = (await page.evaluate(() => window.allocationFixture.measure())).mode;
    await page.evaluate(scale => window.allocationFixture.reassign('compact', scale, true), scale);
    await page.evaluate(scale => window.allocationFixture.reassign(undefined, scale), scale);
    if ((await page.evaluate(() => window.allocationFixture.measure())).mode !== 'compact') throw new Error(`${context}: width silently restored presentation`);
    await page.evaluate(scale => window.allocationFixture.reassign('inspect', scale), scale);
    const restored = await page.evaluate(() => window.allocationFixture.measure());
    if (restored.edit !== result.edit || restored.failures.length) throw new Error(`${context}: Restore lost state/containment: ${restored.failures.join('; ')}`);
    if (remembered === 'collection' && restored.mode !== 'collection') throw new Error(`${context}: forgot Collection`);
    cases++;
  }
  let compactCases = 0;
  for (const kind of ['generic', 'loader']) for (const scale of [1, 1.25, 2]) for (const [width, height] of [[96, 480], [800, 64], [80, 64]]) {
    const result = await page.evaluate(args => window.allocationFixture.mountCompactAspect(...args), [kind, width, height, scale]);
    const context = `${kind} Compact ${width}x${height} @${scale}`;
    if (result.failures.length) throw new Error(`${context}: ${result.failures.join('; ')}`);
    if (Math.abs(result.header.height - result.node.height) > 1 || Math.abs(result.identity.bottom - result.node.bottom) > 1) throw new Error(`${context}: unused Compact height`);
    if (width < 320) {
      if (!result.stacked || !result.wrapped || result.close.bottom > result.identity.top + 1 || Math.abs(result.identity.width - result.node.width) > 1) throw new Error(`${context}: Close steals identity width`);
      if (height === 480 && !result.textFits) throw new Error(`${context}: identity text clipped despite tall allocation`);
    }
    const identity = page.locator('[data-node-title-bar] > button');
    await identity.click({ position: { x: result.identity.width / 2, y: result.identity.height - 4 } });
    const close = page.locator('[data-action-id="CloseOccurrence.VisualizerOperator"]'); await close.click();
    const invocations = await page.evaluate(() => { const node = document.body.firstElementChild; return { restore: node.dataset.restoreCount, close: node.dataset.closeCount }; });
    if (invocations.restore !== '1' || invocations.close !== '1') throw new Error(`${context}: Restore and Close activation overlap`);
    if (result.stacked) {
      const header = page.locator('[data-node-title-header]');
      await header.click({ position: { x: 4, y: result.close.height / 2 } });
      if (await page.evaluate(() => document.body.firstElementChild.dataset.restoreCount) !== '2') throw new Error(`${context}: dead space beside Close`);
    }
    const beforeKeyboard = await page.evaluate(() => Number(document.body.firstElementChild.dataset.restoreCount));
    await identity.focus(); await page.keyboard.press('Enter');
    if (await page.evaluate(() => Number(document.body.firstElementChild.dataset.restoreCount)) !== beforeKeyboard + 1) throw new Error(`${context}: keyboard Restore failed`);
    compactCases++;
  }
  for (const height of [1000, 1450]) {
    const result = await page.evaluate(height => window.allocationFixture.mountVerticalPath(height), height);
    if (result.modes.join(',') !== 'compact,collection,properties') throw new Error(`Vertical path: wrong modes ${result.modes}`);
    if (result.contextViewportHeight < 100) throw new Error(`Vertical path: Collection context squeezed to ${result.contextViewportHeight}px`);
    if (result.bounds[2].height < 480 || result.bounds[1].height < 320) throw new Error('Vertical path shrank below useful minima');
    if (result.connectors !== 2 || result.scale !== 1) throw new Error('Vertical path changed topology or auto-zoomed');
    for (let index = 1; index < 3; index++) if (result.bounds[index].y <= result.bounds[index - 1].y + result.bounds[index - 1].height) throw new Error('Vertical path lost its connector channel');
    if (height === 1450 && result.contentHeight > result.viewportHeight + 1) throw new Error('Preferred reference footprint does not fit its allocation');
    const restored = await page.evaluate(() => window.allocationFixture.restoreVerticalRoot());
    if (restored.modes.join(',') !== 'collection,compact,compact') throw new Error(`Restore left descendants expanded: ${restored.modes}`);
    const width = await page.evaluate(() => window.allocationFixture.narrowPathViewport());
    if (width.before !== width.after) throw new Error(`Single column shrank with unused space: ${width.before} -> ${width.after}`);
  }
  const horizontal = await page.evaluate(() => window.allocationFixture.mountHorizontalPath());
  if (!horizontal.inbound.some(([source, target]) => source === '0' && target === '1') || horizontal.connectors !== 3) throw new Error('Missing actual inbound edge at left boundary');
  for (const id of ['2', '1', '0', '1', '2', '3']) {
    const modes = await page.evaluate(id => window.allocationFixture.selectPathOccurrence(id), id);
    if (modes.some((mode, index) => mode !== (String(index) === id ? 'properties' : 'compact'))) throw new Error(`Selection ${id} accumulated expanded modes: ${modes}`);
  }
  const descended = await page.evaluate(() => window.allocationFixture.descendFromRestoredMiddle());
  if (descended.modes.some(([id, mode]) => mode !== (id === 'down' ? 'properties' : id === '1' ? 'collection' : 'compact'))) throw new Error(`Vertical traversal revived compressed ancestors: ${JSON.stringify(descended.modes)}`);
  if (descended.widths.some(([before, after]) => before !== after)) throw new Error('Vertical traversal widened an unrelated compressed column');
  for (const scale of [1, 1.25, 2]) {
    const tabs = await page.evaluate(scale => window.allocationFixture.mountCollectionsIndex(scale), scale);
    if (tabs.count !== 4 || tabs.more) throw new Error(`Collection index wastes horizontal allocation @${scale}: ${JSON.stringify(tabs)}`);
    await page.getByRole('button', { name: 'Expand Properties pane', exact: true }).click();
    await page.evaluate(() => new Promise(requestAnimationFrame)); await page.evaluate(() => new Promise(requestAnimationFrame));
    await page.getByRole('button', { name: 'Restore Properties pane', exact: true }).click();
    await page.evaluate(() => new Promise(requestAnimationFrame)); await page.evaluate(() => new Promise(requestAnimationFrame));
    const afterProperties = await page.evaluate(() => window.allocationFixture.collectionIndexState());
    if (afterProperties.count !== 4 || afterProperties.more) throw new Error(`Restored Collection index wastes horizontal allocation @${scale}: ${JSON.stringify(afterProperties)}`);
    await page.getByRole('tab', { name: /^Available Schemas/ }).click();
    const collection = await page.evaluate(() => window.allocationFixture.collectionIndexState());
    if (collection.count !== 4 || collection.more || collection.pane) throw new Error(`Collection mode omitted its index: ${JSON.stringify(collection)}`);
    await page.getByRole('tab', { name: /^Offers Theme/ }).click();
    const switched = await page.evaluate(() => window.allocationFixture.collectionIndexState());
    if (!switched.selected?.startsWith('Offers Theme') || switched.pane) throw new Error('Collection group cannot switch collections');
    if (!await page.getByRole('button', { name: 'Properties, Actions & Relationships', exact: true }).isVisible()) throw new Error('Switching collections hid the Inspection return');
    await page.evaluate(scale => window.allocationFixture.reassign(undefined, scale), scale);
    await page.evaluate(() => new Promise(requestAnimationFrame)); await page.evaluate(() => new Promise(requestAnimationFrame));
    const narrow = await page.evaluate(() => window.allocationFixture.collectionIndexState());
    if (!narrow.more || narrow.count >= 4) throw new Error('Narrow Collection index lost responsive disclosure');
    await page.evaluate(scale => { const node = document.body.firstElementChild; node.visualizerOperations['ApplyNodePresentationV2.VisualizerOperator']({ width: 800 * scale, height: 480 * scale }); }, scale);
    await page.evaluate(() => new Promise(requestAnimationFrame)); await page.evaluate(() => new Promise(requestAnimationFrame));
    const wide = await page.evaluate(() => window.allocationFixture.collectionIndexState());
    if (wide.count !== 4 || wide.more) throw new Error('Collection index did not reuse its wider grant');
  }
  if (process.env.MAP_ALLOCATION_SCREENSHOT) {
    if (process.env.MAP_ALLOCATION_SCREENSHOT_MODE === 'compact-tall') await page.evaluate(() => window.allocationFixture.mountCompactAspect('generic', 96, 480, 1));
    else if (process.env.MAP_ALLOCATION_SCREENSHOT_MODE === 'edge') await page.evaluate(() => window.allocationFixture.mountHorizontalPath());
    else await page.evaluate(mode => window.allocationFixture.mount('generic', mode, 1, mode === 'collection', 'preferred'), process.env.MAP_ALLOCATION_SCREENSHOT_MODE || 'properties');
    // Preview the authored surfaces against both shipped palettes, not fixture grays.
    const themeSource = await readFile(join(root, 'schema-src/theme/schema.tdl'), 'utf8');
    for (const theme of ['DeepOcean', 'WarmParchment']) {
      const properties = {};
      const assignments = new RegExp(`instance Demo1\\.${theme}Theme[^ ]+ \\{([\\s\\S]*?)\\n\\}`, 'g');
      for (const match of themeSource.matchAll(assignments)) {
        const value = match[1].match(/PresentationValue "([^"\n]+)"/)?.[1];
        const token = match[1].match(/ForDesignToken -> ([A-Za-z]+)\.DesignToken/)?.[1];
        if (token && value) properties[`--dahn-${token.replace(/[A-Z]/g, letter => `-${letter.toLowerCase()}`).slice(1)}`] = value;
      }
      await page.evaluate(properties => {
        for (const [name, value] of Object.entries(properties)) document.body.style.setProperty(name, value);
        document.body.style.background = 'var(--dahn-canvas-surface-background)';
        document.body.style.color = 'var(--dahn-canvas-text-color)';
      }, properties);
      const path = theme === 'DeepOcean' ? process.env.MAP_ALLOCATION_SCREENSHOT : process.env.MAP_ALLOCATION_SCREENSHOT.replace(/\.png$/, '-warm.png');
      await page.screenshot({ path });
    }
  }
  if (errors.length) throw new Error(errors.join('\n'));
  console.log(`${cases} browser allocation/containment/keyboard cases, ${compactCases} Compact aspect-ratio cases and composed navigation/Collection-group cases passed`);
} finally {
  await browser?.close(); await new Promise(resolve => server.close(resolve)); await rm(directory, { recursive: true, force: true });
}
