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
      const member = page.getByRole('button', { name: 'Open Collection member 29', exact: true });
      await member.focus(); await member.evaluate(element => element.scrollIntoView({ block: 'nearest' }));
      if (!await member.evaluate(element => { const r = element.getBoundingClientRect(); return document.activeElement === element && element.contains(document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2)); })) throw new Error(`${context}: collection navigation is obscured`);
      await page.getByRole('button', { name: 'Properties', exact: true }).click();
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
  for (const height of [1000, 1450]) {
    const result = await page.evaluate(height => window.allocationFixture.mountVerticalPath(height), height);
    if (result.modes.join(',') !== 'compact,collection,properties') throw new Error(`Vertical path: wrong modes ${result.modes}`);
    if (result.contextViewportHeight < 100) throw new Error(`Vertical path: Collection context squeezed to ${result.contextViewportHeight}px`);
    if (result.bounds[2].height < 480 || result.bounds[1].height < 320) throw new Error('Vertical path shrank below useful minima');
    if (result.connectors !== 2 || result.scale !== 1) throw new Error('Vertical path changed topology or auto-zoomed');
    for (let index = 1; index < 3; index++) if (result.bounds[index].y <= result.bounds[index - 1].y + result.bounds[index - 1].height) throw new Error('Vertical path lost its connector channel');
    if (height === 1450 && result.contentHeight > result.viewportHeight + 1) throw new Error('Preferred reference footprint does not fit its allocation');
  }
  if (process.env.MAP_ALLOCATION_SCREENSHOT) {
    await page.evaluate(mode => window.allocationFixture.mount('generic', mode, 1, mode === 'collection', 'preferred'), process.env.MAP_ALLOCATION_SCREENSHOT_MODE || 'properties');
    await page.screenshot({ path: process.env.MAP_ALLOCATION_SCREENSHOT });
  }
  if (errors.length) throw new Error(errors.join('\n'));
  console.log(`${cases} browser allocation/containment/keyboard cases and 2 composed vertical-path cases passed`);
} finally {
  await browser?.close(); await new Promise(resolve => server.close(resolve)); await rm(directory, { recursive: true, force: true });
}
