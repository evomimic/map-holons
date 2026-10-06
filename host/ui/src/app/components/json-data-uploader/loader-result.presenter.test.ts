import { describe, expect, it, vi } from 'vitest';
import type {
  BaseValue,
  HolonCollection,
  HolonId,
  HolonReference,
  ReadableHolon,
} from '../../../dahn/deps/map-sdk';
import { presentLoaderResult } from './loader-result.presenter';

function holonMock(
  properties: Record<string, BaseValue>,
  relatedErrors: Array<Record<string, BaseValue>> = [],
  relatedHolons?: ReadableHolon['relatedHolons'],
): ReadableHolon {
  const members = relatedErrors.map(
    (error) => holonMock(error) as unknown as HolonReference,
  );
  const relatedCollection = {
    length: members.length,
    members,
    getByKey: vi.fn<(key: string) => HolonReference | undefined>(),
    [Symbol.iterator]: function* (): Iterator<HolonReference> {
      for (const member of members) {
        yield member;
      }
    },
  } as unknown as HolonCollection;

  return {
    cloneHolon: vi.fn<() => Promise<never>>(),
    summarize: vi.fn<() => Promise<string>>(),
    holonId: vi.fn<() => Promise<HolonId>>(),
    predecessor: vi.fn<() => Promise<HolonReference | null>>(),
    key: vi.fn<() => Promise<string | null>>(),
    versionedKey: vi.fn<() => Promise<string>>(),
    propertyValue: vi.fn<(name: string) => Promise<BaseValue | null>>(
      async (name: string) => properties[name] ?? null,
    ),
    relatedHolons:
      relatedHolons ??
      vi.fn<(name: string) => Promise<HolonCollection>>(async () => relatedCollection),
  };
}

describe('presentLoaderResult', () => {
  const response = (status: string, committed: number | null = 0) => holonMock({
    LoadCommitStatus: { StringValue: status },
    ErrorCount: { IntegerValue: 0 },
    ValidationViolationCount: { IntegerValue: 2 },
    HolonsStaged: { IntegerValue: 9 },
    ...(committed === null ? {} : { HolonsCommitted: { IntegerValue: committed } }),
  });

  it.each([
    ['Complete', 2, 'Load complete'],
    ['Rejected', 0, 'Load rejected'],
    ['Incomplete', 2, 'Load incomplete — some holons were saved'],
    ['Incomplete', 0, 'Load incomplete — no holons were saved'],
    ['Incomplete', null, 'Load incomplete — saved count is unavailable'],
    ['Skipped', 0, 'Load skipped'],
    ['Unknown', 0, 'Load could not be completed'],
  ])('presents %s independently of operational errors', async (status, count, outcome) => {
    const result = await presentLoaderResult(response(status, count));
    expect(result.outcome).toBe(outcome);
    expect(result.validationViolationCount).toBe('2');
    expect(result.errorCount).toBe('0');
  });

  it('requires empty-input evidence for Nothing to load', async () => {
    const result = await presentLoaderResult(holonMock({
      LoadCommitStatus: { StringValue: 'Skipped' }, TotalLoaderHolons: { IntegerValue: 0 },
      ErrorCount: { IntegerValue: 0 }, ValidationViolationCount: { IntegerValue: 0 },
    }));
    expect(result.outcome).toBe('Nothing to load');
  });

  it('preserves status and other counts when one field and diagnostics fail', async () => {
    const holon = response('Complete', 2);
    const original = holon.propertyValue;
    holon.propertyValue = vi.fn(async name => {
      if (name === 'HolonsCommitted') throw new Error('count unavailable');
      return original(name);
    });
    holon.relatedHolons = vi.fn(async () => { throw new Error('diagnostics unavailable'); });
    const result = await presentLoaderResult(holon);
    expect(result.outcome).toBe('Load complete');
    expect(result.holonsCommitted).toBe('Not available');
    expect(result.holonsStaged).toBe('9');
    expect(result.readFailures).toEqual(expect.arrayContaining([
      'HolonsCommitted: count unavailable', 'HasLoadError: diagnostics unavailable',
    ]));
  });

  it('keeps diagnostics on Complete and missing source values explicitly unavailable', async () => {
    const result = await presentLoaderResult(holonMock({ LoadCommitStatus: { StringValue: 'Complete' } }, [
      { ErrorMessage: { StringValue: 'a diagnostic' } },
    ]));
    expect(result.loadErrors[0]).toMatchObject({ errorMessage: 'a diagnostic', filename: 'Not available' });
    expect(result.outcome).toBe('Load complete');
  });
  it('does not convert malformed counts into evidence of zero persistence', async () => {
    const result = await presentLoaderResult(holonMock({
      LoadCommitStatus: { StringValue: 'Incomplete' }, HolonsCommitted: { StringValue: '0' },
    }));
    expect(result.outcome).toBe('Load incomplete — saved count is unavailable');
    expect(result.readFailures).toHaveLength(1);
  });

  it('does not label skipped work empty when diagnostic evidence is unavailable', async () => {
    const holon = holonMock({
      LoadCommitStatus: { StringValue: 'Skipped' }, TotalLoaderHolons: { IntegerValue: 0 },
      ErrorCount: { IntegerValue: 0 }, ValidationViolationCount: { IntegerValue: 0 },
    }, [], vi.fn(async () => { throw new Error('unreadable'); }));
    expect((await presentLoaderResult(holon)).outcome).toBe('Load skipped');
  });

});
