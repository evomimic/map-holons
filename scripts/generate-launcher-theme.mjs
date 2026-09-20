import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const root = new URL('../', import.meta.url);
async function holons(path) {
  return JSON.parse(await readFile(new URL(path, root), 'utf8')).holons;
}
const themes = await holons('generated/json-imports/theme/schema.json');
const tokens = await holons('generated/json-imports/design-tokens/schema.json');
const byKey = new Map([...themes, ...tokens].map(holon => [holon.key, holon]));
function related(holon, name) {
  const relationship = holon.relationships.find(item => item.name === name);
  if (!relationship) throw new Error(`Missing ${name} on ${holon.key}`);
  return relationship.target.map(target => {
    const found = byKey.get(target.$ref);
    if (!found) throw new Error(`Unresolved ${target.$ref}`);
    return found;
  });
}
// Static presentation for the launcher before persisted runtime handles exist.
// Canvas still projects its selected, persisted Theme through the Reference Layer.
const theme = byKey.get('MAP.BootstrapTheme');
if (!theme) throw new Error('Missing bootstrap theme');
const values = related(theme, 'HasThemeTokenAssignment').map(assignment => {
  const targets = related(assignment, 'ForDesignToken');
  if (targets.length !== 1) throw new Error('Expected one token per assignment');
  const name = targets[0].properties.DesignTokenName;
  const value = assignment.properties.PresentationValue;
  if (!/^[A-Za-z][A-Za-z0-9]*$/.test(name) || typeof value !== 'string' || /[;{}@]/.test(value)) {
    throw new Error('Unsafe launcher theme value');
  }
  return `  --dahn-${name.replace(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase()}: ${value};`;
}).sort();
await writeFile(fileURLToPath(new URL('host/ui/src/launcher-theme.generated.css', root)),
  '/* Generated from MAP.BootstrapTheme. Run npm run map-schema:bootstrap-bundle. */\n:root {\n' + values.join('\n') + '\n}\n');
