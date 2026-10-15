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
  for (const kind of ['generic', 'loader']) for (const scale of [1, 1.25, 2]) for (const horizontal of ['compact', 'partial', 'expanded']) for (const vertical of ['compact', 'partial', 'expanded']) {
    await page.evaluate(args => window.allocationFixture.mount(...args), [kind, horizontal, vertical, scale]);
    const result = await page.evaluate(() => window.allocationFixture.measure());
    if (result.failures.length) throw new Error(`${kind} ${horizontal}/${vertical} @${scale}: ${result.failures.join('; ')}`);
    // The identity Restore remains reachable at every non-inspect assignment.
    if (!result.capabilities.includes('inspect')) {
      const identity = page.locator('[data-node-title-bar] > button'); await identity.focus(); await page.keyboard.press('Enter');
      if (!(await page.evaluate(() => window.allocationFixture.measure())).restored) throw new Error('Keyboard Restore failed');
    }
    // Required navigation remains focusable and hittable after local scrolling.
    if (result.capabilities.includes('navigate-plural') || result.capabilities.includes('inspect')) {
      const member = page.getByRole('button', { name: 'Open Collection member 29', exact: true });
      await member.focus();
      await member.evaluate(element => element.scrollIntoView({ block: 'nearest', inline: 'nearest' }));
      const hittable = await member.evaluate(element => {
        const rect = element.getBoundingClientRect();
        const hit = document.elementFromPoint(rect.left + rect.width / 2, rect.top + rect.height / 2);
        return document.activeElement === element && !!hit && element.contains(hit);
      });
      if (!hittable) throw new Error(`${kind} ${horizontal}/${vertical} @${scale}: Collection navigation is clipped or obscured`);
    }
    if (result.capabilities.includes('navigate-singular') || result.capabilities.includes('inspect')) {
      const singular = page.getByRole('button', { name: 'Navigate singular relationship', exact: true });
      if (!await singular.isVisible()) await page.getByRole('button', { name: 'More relationships', exact: true }).click();
      await singular.focus();
      await singular.evaluate(element => element.scrollIntoView({ block: 'center', inline: 'nearest' }));
      const hittable = await singular.evaluate(element => {
        const rect = element.getBoundingClientRect();
        const hit = document.elementFromPoint(rect.left + rect.width / 2, rect.top + rect.height / 2);
        return document.activeElement === element && !!hit && element.contains(hit);
      });
      if (!hittable) throw new Error(`${kind} ${horizontal}/${vertical} @${scale}: Singular navigation is clipped or obscured`);
    }
    // Compression and restoration reuse the exact selected content and edit.
    await page.evaluate(scale => window.allocationFixture.reassign('compact', 'compact', scale), scale);
    await page.evaluate(scale => window.allocationFixture.reassign('expanded', 'expanded', scale), scale);
    const restored = await page.evaluate(() => window.allocationFixture.measure());
    if (restored.edit !== result.edit || restored.failures.length) throw new Error('Resize lost content or containment');
    for (let step = 0; step < 12; step++) await page.keyboard.press('Tab');
    if (!result.edit.startsWith('retained-')) throw new Error('Retained edit state was lost');
    cases++;
  }
  if (errors.length) throw new Error(errors.join('\n'));
  console.log(`${cases} browser allocation/containment/keyboard cases passed`);
} finally {
  await browser?.close(); await new Promise(resolve => server.close(resolve)); await rm(directory, { recursive: true, force: true });
}
