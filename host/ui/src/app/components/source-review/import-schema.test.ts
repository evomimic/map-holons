import '@angular/compiler';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { loadImportValidator } from './import-schema';
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => false }));
afterEach(() => vi.unstubAllGlobals());
const respond = (text: string, ok = true) => vi.stubGlobal('fetch', vi.fn(async () => ({ ok, status: 503, text: async () => text })));

describe('import schema snapshot', () => {
  it('validates syntax and the real supported schema with diagnostics', async () => {
    respond(readFileSync('ui/public/bootstrap-import.schema.json', 'utf8'));
    const validate = await loadImportValidator();
    expect((await validate('{"holons":[]}')).valid).toBe(true);
    expect((await validate('{')).diagnostics.length).toBeGreaterThan(0);
    const invalid = await validate('{"wrong":true}');
    expect(invalid.valid).toBe(false);
    expect(invalid.diagnostics.length).toBeGreaterThan(0);
  });
  it.each(['{', '[]', '{"type":"not-a-schema-type"}'])('rejects malformed schema %s', async text => {
    respond(text);
    await expect(loadImportValidator()).rejects.toThrow();
  });
  it('reports schema read failure', async () => {
    respond('', false);
    await expect(loadImportValidator()).rejects.toThrow('503');
  });
  it('does not reuse a name-only cache across changed schemas', async () => {
    respond('{"type":"string"}');
    const first = await loadImportValidator();
    respond('{"type":"number"}');
    const second = await loadImportValidator();
    expect((await first('1')).valid).toBe(false);
    expect((await second('1')).valid).toBe(true);
  });
});
