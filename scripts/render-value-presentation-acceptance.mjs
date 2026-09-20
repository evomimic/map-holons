import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import ts from 'typescript';

// Consume the verified artifacts exported by book_value_presentation_acceptance.
// This page deliberately has no registry of semantic candidates or fallback.
const [evidencePath, outputPath] = process.argv.slice(2);
if (!evidencePath || !outputPath) throw new Error('Usage: node scripts/render-value-presentation-acceptance.mjs <evidence.json> <output.html>');
const evidence = JSON.parse(await readFile(evidencePath, 'utf8'));
const theme = await readFile(resolve('host/ui/src/launcher-theme.generated.css'), 'utf8');
const canvasModule = ts.transpileModule(await readFile(resolve('host/ui/src/dahn/canvas/create-canvas-root.ts'), 'utf8'), { compilerOptions: { module: ts.ModuleKind.ESNext } }).outputText;
const pathSource = await readFile(resolve('host/conductora/resources/dahn-visualizers/path-inspector.js'), 'utf8');
const layoutChecks = await readFile(resolve('scripts/value-presentation-layout-checks.mjs'), 'utf8');
const serialized = JSON.stringify(evidence).replaceAll('<', '\\u003c');
await writeFile(outputPath, `<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Issue 706 — committed Book acceptance</title>
<style>${theme}
body { margin: 0; }
* { box-sizing: border-box; } #result { height: 100vh; }
</style><main id="result"></main>
<script type="module">
const evidence = ${serialized};
const modules = new Map();
let nextId = 0;
async function realize(module) {
  let tag = modules.get(module.source);
  if (!tag) {
    const url = URL.createObjectURL(new Blob([module.source], { type: 'text/javascript' }));
    try {
      const exports = await import(url);
      tag = 'acceptance-selected-' + nextId++;
      customElements.define(tag, exports[module.entrypoint]);
      modules.set(module.source, tag);
    } finally { URL.revokeObjectURL(url); }
  }
  return document.createElement(tag);
}
const children = new Map();
for (const field of evidence.fields) {
  const value = await realize(field.visualizer);
  value.setContext({ propertyPresentation: { propertyName: field.name, value: field.value } });
  const property = await realize(field.property);
  property.setContext({ propertyPresentation: { propertyName: field.name }, childVisualizers: new Map([['value', value]]) });
  children.set(field.name, property);
}
const properties = await realize(evidence.properties);
properties.setContext({ childVisualizers: children });
const node = await realize(evidence.node);
node.setContext({ title: 'Book.InverseSchema.Instance', childVisualizers: new Map([['properties', properties]]) });
const canvasSource = ${JSON.stringify(canvasModule).replaceAll('<', '\\u003c')};
const canvasUrl = URL.createObjectURL(new Blob([canvasSource], { type: 'text/javascript' }));
const { createCanvasRoot } = await import(canvasUrl);
URL.revokeObjectURL(canvasUrl);
const canvas = createCanvasRoot(document.querySelector('#result'));
canvas.awaitingHomeDancer.hidden = true;
const path = await realize({ source: ${JSON.stringify(pathSource).replaceAll('<', '\\u003c')}, entrypoint: 'default' });
path.setContext({ title: 'SpaceNavigator.Dancer', childVisualizers: new Map([['root-node', node]]) });
canvas.primarySlot.append(path);
document.body.dataset.acceptance = 'rendered';
if (new URLSearchParams(location.search).has('check-layout')) {
  const checkUrl = URL.createObjectURL(new Blob([${JSON.stringify(layoutChecks).replaceAll('<', '\\u003c')}], { type: 'text/javascript' }));
  const report = document.createElement('pre');
  report.id = 'layout-check-results';
  try {
    const { checkValuePresentationLayout } = await import(checkUrl);
    report.textContent = 'PASS\\n' + (await checkValuePresentationLayout(document.querySelector('#result'))).join('\\n');
    report.dataset.result = 'pass';
  } catch (error) {
    report.textContent = 'FAIL: ' + error.message;
    report.dataset.result = 'fail';
  } finally { URL.revokeObjectURL(checkUrl); }
  document.body.append(report);
}
</script></html>`);
console.log(outputPath);
