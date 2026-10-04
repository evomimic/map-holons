import '@angular/compiler';
import { ChangeDetectorRef } from '@angular/core';
import { describe, expect, it, vi } from 'vitest';
import { JsonDataUploader } from './json-data-uploader.component';
import { DomainError, type HolonReference } from '../../../dahn/deps/map-sdk';
import { SchemaValidatorService } from '../../services/schema-validation.service';
import { ContentController } from '../../contollers/content.controller';

const load = vi.hoisted(() => vi.fn());
vi.mock('../../../dahn/deps/map-sdk', async original => ({
  ...await original<object>(),
  MapClient: class { async beginTransaction() { return { loadHolons: load }; } },
}));

function uploader() {
  const component = new JsonDataUploader({} as SchemaValidatorService, {} as ContentController,
    { markForCheck: vi.fn() } as unknown as ChangeDetectorRef);
  component.standaloneMode = true;
  component.schemaJson.set('{}');
  component.dataFiles.set([{ id: '1', filename: 'source.json', displayName: 'source.json',
    content: '{"holons":[]}', validationResult: { valid: true, errors: [] } }]);
  return component;
}
function response(status: string) {
  return {
    propertyValue: vi.fn(async (name: string) => name === 'LoadCommitStatus' ? { StringValue: status }
      : name === 'ValidationViolationCount' ? { IntegerValue: status === 'Rejected' ? 2 : 0 }
      : name === 'ErrorCount' ? { IntegerValue: 0 } : null),
    relatedHolons: vi.fn(async () => ({ members: [] })),
  } as unknown as HolonReference;
}

describe('loader submission outcome', () => {
  it('retains sources and schema on zero-operational-error rejection', async () => {
    load.mockResolvedValue(response('Rejected'));
    const component = uploader();
    await component.savetohost();
    expect(component.loaderResultStatus).toBe('Load rejected');
    expect(component.successMessage).toBe('');
    expect(component.schemaJson()).toBe('{}');
    expect(component.dataFiles()).toHaveLength(1);
    expect(component.isLoading).toBe(false);
  });
  it('replaces pending feedback and preserves structured failure details', async () => {
    load.mockRejectedValue(new DomainError('LoaderParsingError', 'source.json: malformed record'));
    const component = uploader();
    await component.savetohost();
    expect(component.loaderResultStatus).toBe('Load could not be completed');
    expect(component.errorMessage).toContain('source.json: malformed record');
    expect(component.loaderResult).toBeNull();
    expect(component.isLoading).toBe(false);
    expect(component.dataFiles()).toHaveLength(1);
  });
  it('preserves the existing successful submission flow', async () => {
    load.mockResolvedValue(response('Complete'));
    const component = uploader();
    await component.savetohost();
    expect(component.successMessage).toBe('Load complete');
    expect(component.dataFiles()).toEqual([]);
    expect(component.isLoading).toBe(false);
  });
  it('keeps sources when the response status is unavailable', async () => {
    load.mockResolvedValue(response('Unknown'));
    const component = uploader();
    await component.savetohost();
    expect(component.successMessage).toBe('');
    expect(component.dataFiles()).toHaveLength(1);
    expect(component.loaderResult?.readFailures).not.toEqual([]);
  });
});
