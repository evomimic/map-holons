import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';

// Consume the verified artifacts exported by book_value_presentation_acceptance.
// This page deliberately has no registry of semantic candidates or fallback.
const [evidencePath, outputPath] = process.argv.slice(2);
if (!evidencePath || !outputPath) throw new Error('Usage: node scripts/render-value-presentation-acceptance.mjs <evidence.json> <output.html>');
const evidence = JSON.parse(await readFile(evidencePath, 'utf8'));
const theme = await readFile(resolve('host/ui/src/launcher-theme.generated.css'), 'utf8');
const serialized = JSON.stringify(evidence).replaceAll('<', '\\u003c');
await writeFile(outputPath, `<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Issue 706 — committed Book acceptance</title>
<style>${theme}
body { margin: 24px; background: var(--dahn-canvas-surface-background); color: var(--dahn-canvas-text-color); font-family: var(--dahn-canvas-font-family); }
* { box-sizing: border-box; } main { max-width: 1100px; margin: auto; } h1 { font-size: 20px; } p { margin-bottom: 24px; }
</style><main><h1>Committed Book — read-only scalar presentation</h1><p>Descriptor discovery, selection and verified artifacts supplied by the Holochain acceptance test.</p><div id="result"></div></main>
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
document.querySelector('#result').append(node);
document.body.dataset.acceptance = 'rendered';
</script></html>`);
console.log(outputPath);
