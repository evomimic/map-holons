import { isTauri } from '@tauri-apps/api/core';
import { resolveResource } from '@tauri-apps/api/path';
import { readTextFile } from '@tauri-apps/plugin-fs';
import { SchemaValidatorService } from '../../services/schema-validation.service';
import type { SourceValidator } from './source-review-state';

/** A fresh validator per schema snapshot avoids the legacy name-only cache across revisions. */
export async function loadImportValidator(): Promise<SourceValidator> {
  let text: string;
  if (isTauri()) {
    text = await readTextFile(await resolveResource('resources/bootstrap-import.schema.json'));
  } else {
    const response = await fetch('/bootstrap-import.schema.json');
    if (!response.ok) throw new Error(`Import schema could not be read (${response.status})`);
    text = await response.text();
  }
  const schema: unknown = JSON.parse(text);
  if (!schema || typeof schema !== 'object' || Array.isArray(schema)) throw new Error('Import schema must be an object');
  const validator = new SchemaValidatorService();
  validator.compileSchema('loader-review', schema);
  return content => {
    const result = validator.validateJsonString('loader-review', content);
    return { valid: result.valid, diagnostics: [
      ...(result.parseError ? [result.parseError] : []),
      ...(result.errors ?? []).map(error => `${error.path}: ${error.message}`),
    ] };
  };
}
