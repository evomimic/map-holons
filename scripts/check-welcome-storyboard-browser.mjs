/** Verify the shipped Welcome page against controlled backend status responses.
 * Set MAP_PLAYWRIGHT_MODULE to an installed Playwright module when it is not local.
 */
import { readFile, mkdir } from 'node:fs/promises';
import { createServer } from 'node:http';
import { resolve, join } from 'node:path';
import { tmpdir } from 'node:os';
const { chromium } = await import(process.env.MAP_PLAYWRIGHT_MODULE || 'playwright');
const root = resolve(import.meta.dirname, '../host/crates/welcome/public');
const output = process.env.MAP_STORYBOARD_OUTPUT || join(tmpdir(), 'map-welcome-storyboard-review');
await mkdir(output, { recursive: true });
const server = createServer(async (request, response) => {
  const route = new URL(request.url, 'http://localhost').pathname;
  const files = { '/': ['welcome.html', 'text/html'], '/welcome.js': ['welcome.js', 'text/javascript'], '/welcome.css': ['welcome.css', 'text/css'], '/images/welcome-garden.png': ['images/welcome-garden.png', 'image/png'] };
  if (!files[route]) { response.writeHead(404).end(); return; }
  try { response.setHeader('content-type', files[route][1]); response.end(await readFile(join(root, files[route][0]))); }
  catch (error) { response.writeHead(500).end(String(error)); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const initial = { phase: 'initializing', stage: 'initializing-host', constitution_available: false, environment_management_available: true, message: 'Starting the trusted local runtime.' };
const preparing = { ...initial, phase: 'provisioning', stage: 'loading-core-schema', preparing_new_space: true };
const naming = { ...initial, phase: 'foundation-present', constitution_available: true, preparing_new_space: true, message: 'Ready for personal setup.' };
const returning = { ...initial, phase: 'association-present', navigator_available: true, new_environment_available: true, message: 'Your saved Space is ready.' };
let browser;
try {
  browser = await chromium.launch({ channel: 'chrome', headless: true });
  const context = await browser.newContext({ viewport: { width: 1060, height: 820 }, reducedMotion: 'reduce' });
  const errors = [];
  async function pageFor(status) {
    const page = await context.newPage();
    await page.clock.install();
    page.on('pageerror', error => errors.push(error.message));
    await page.addInitScript(({ status, returning }) => {
      window.storyboardFixture = { status, calls: [] };
      window.__TAURI__ = { core: { invoke: async (command, input) => {
        window.storyboardFixture.calls.push([command, input]);
        if (command === 'welcome_status') return window.storyboardFixture.status;
        if (command === 'constitute_personal_space') { window.storyboardFixture.status = returning; return 'complete'; }
        if (command === 'saved_environments') return [];
        throw new Error(`Unexpected command: ${command}`);
      } } };
    }, { status, returning });
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    await page.waitForFunction(() => window.storyboardFixture.calls.length > 0);
    return page;
  }
  async function verify(page, view, filename) {
    await page.waitForFunction(view => document.body.dataset.view === view, view);
    await page.evaluate(() => document.fonts.ready);
    const layout = await page.evaluate(() => {
      const primary = [...document.querySelectorAll('#constitute, #open-navigator')].filter(button => !button.hidden);
      return { overflow: document.documentElement.scrollWidth > innerWidth + 1,
        timerHidden: document.getElementById('activity-elapsed').getBoundingClientRect().height === 0,
        hiddenActionVisible: ['discovery', 'invitation', 'privacy', 'finalizing', 'problem', 'recovery'].includes(document.body.dataset.view) && primary.length > 0,
        clipped: primary.some(button => button.getBoundingClientRect().bottom > innerHeight),
        measuredProgress: document.getElementById('preparation-progress').hasAttribute('aria-valuenow') };
    });
    if (Object.values(layout).some(Boolean)) throw new Error(`${filename}: ${JSON.stringify(layout)}`);
    await page.screenshot({ path: join(output, `${filename}.png`), fullPage: true });
  }
  const discovery = await pageFor(initial); await verify(discovery, 'discovery', '01-discovery'); await discovery.close();
  const preparation = await pageFor(preparing);
  await verify(preparation, 'invitation', '02-invitation');
  if (await preparation.locator('#usual-way input, #usual-way button').count()) throw new Error('Comparison illustration has interactive account controls');
  await preparation.clock.runFor(14000);
  if (await preparation.evaluate(() => document.body.dataset.view !== 'invitation')) throw new Error('Comparison disappeared before its reading interval');
  if (await preparation.locator('#startup-details').getAttribute('open') !== null) throw new Error('Setup details unexpectedly expanded');
  if (!await preparation.locator('#activity-elapsed').isVisible()) throw new Error('Collapsed details hide the timer');
  await preparation.clock.runFor(1000);
  await verify(preparation, 'privacy', '03-private-space'); await preparation.close();
  const finalizing = await pageFor({ ...preparing, phase: 'checking-space', stage: 'verifying-space-definitions' });
  await finalizing.clock.runFor(15000);
  await verify(finalizing, 'finalizing', '04-finalizing'); await finalizing.close();
  const personal = await pageFor(naming); await verify(personal, 'naming', '06-naming');
  await personal.locator('#last-name').fill('Storyboard Example'); await personal.locator('#last-name').press('Enter');
  await verify(personal, 'ready', '05-ready');
  if (await personal.evaluate(() => window.storyboardFixture.calls.some(([name]) => name === 'open_space_navigator'))) throw new Error('Ready screen launched automatically');
  await personal.close();
  const existing = await pageFor(returning); await verify(existing, 'returning', '07-returning'); await existing.close();
  const recoveryStatus = { ...initial, phase: 'blocked', stage: 'failed', incompatibility: 'The saved schema version is incompatible', new_environment_available: true };
  const recovery = await pageFor(recoveryStatus); await verify(recovery, 'recovery', '08-recovery');
  if (!await recovery.locator('#new-environment').isVisible()) throw new Error('Recovery hides the explicit fresh-environment choice');
  await recovery.close();
  const problem = await pageFor({ ...initial, phase: 'blocked', stage: 'failed', message: 'Storage could not be opened.' });
  await verify(problem, 'problem', '09-attention'); await problem.close();
  for (const width of [640, 390]) {
    const compact = await pageFor(naming); await compact.setViewportSize({ width, height: width === 640 ? 700 : 844 });
    await verify(compact, 'naming', `naming-${width}`); await compact.close();
  }
  if (errors.length) throw new Error(errors.join('\n'));
  console.log(`Verified nine storyboard screens, responsive naming layouts, explicit entry and recovery. Screenshots: ${output}`);
} finally {
  await browser?.close(); await new Promise(resolve => server.close(resolve));
}
