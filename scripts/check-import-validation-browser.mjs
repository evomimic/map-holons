/** Exercise the real source-review validator under packaged script restrictions. */
import { build } from 'esbuild';
import { createServer } from 'node:http';
import { readFile, mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
const { chromium } = await import(process.env.MAP_PLAYWRIGHT_MODULE || 'playwright');
const root = resolve(import.meta.dirname, '..');
const directory = await mkdtemp(join(tmpdir(), 'map-import-csp-'));
await build({ stdin: { contents: `import '@angular/compiler';
import { loadImportValidator } from './src/app/components/source-review/import-schema';
window.validationProbe = (async () => {
  const validate = await loadImportValidator();
  return {
    valid: await validate(await (await fetch('/example.json')).text()),
    invalid: await validate('{"wrong":true}'),
    malformed: await validate('{'),
  };
})().catch(error => ({ error: error.message }));`, resolveDir: join(root, 'host/ui'), loader: 'ts' },
  bundle: true, platform: 'browser', format: 'esm', target: 'es2022', outfile: join(directory, 'probe.js') });
const server = createServer(async (request, response) => {
  try {
    if (request.url === '/') {
      response.setHeader('content-type', 'text/html');
      response.setHeader('content-security-policy', "default-src 'none'; script-src 'self' blob:; connect-src 'self'");
      response.end('<!doctype html><script type="module" src="/probe.js"></script>'); return;
    }
    const files = {
      '/probe.js': [join(directory, 'probe.js'), 'text/javascript'],
      '/bootstrap-import.schema.json': [join(root, 'host/conductora/resources/bootstrap-import.schema.json'), 'application/json'],
      '/example.json': [join(root, 'generated/json-imports/test/book-person-inverse.json'), 'application/json'],
    };
    const file = files[request.url];
    if (!file) { response.writeHead(404).end(); return; }
    response.setHeader('content-type', file[1]); response.end(await readFile(file[0]));
  } catch (error) { response.writeHead(500).end(String(error)); }
});
let browser;
try {
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  browser = await chromium.launch({ channel: 'chrome', headless: true });
  const page = await browser.newPage();
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  await page.goto(`http://127.0.0.1:${server.address().port}/`);
  await page.waitForFunction(() => window.validationProbe);
  const result = await page.evaluate(() => window.validationProbe);
  if (result.error || !result.valid?.valid || result.invalid?.valid !== false || result.malformed?.valid !== false || errors.length) {
    throw new Error(JSON.stringify({ result, errors }));
  }
  console.log('Packaged CSP permits precompiled validation: book-person-inverse.json passes; invalid JSON and invalid import shapes are rejected.');
} finally {
  await browser?.close(); await new Promise(resolve => server.close(resolve)); await rm(directory, { recursive: true, force: true });
}
