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
    relatedHolons: vi.fn<(name: string) => Promise<HolonCollection>>(
      async () => relatedCollection,
    ),
  };
}

describe('presentLoaderResult', () => {
  it('reads the loader summary without errors', async () => {
    const loaderHolon = holonMock({
      HolonsStaged: { IntegerValue: 3 },
      HolonsCommitted: { IntegerValue: 2 },
      ErrorCount: { IntegerValue: 0 },
      DanceSummary: { StringValue: 'loaded' },
      LinksCreated: { IntegerValue: 7 },
      LoadCommitStatus: { StringValue: 'Committed' },
    });

    await expect(presentLoaderResult(loaderHolon)).resolves.toEqual({
      holonsStaged: '3',
      holonsCommitted: '2',
      errorCount: '0',
      danceSummary: 'loaded',
      linksCreated: '7',
      loadCommitStatus: 'Committed',
      loadErrors: [],
    });
  });

  it('reads related load errors when present', async () => {
    const loaderHolon = holonMock(
      {
        HolonsStaged: { IntegerValue: 8 },
        HolonsCommitted: { IntegerValue: 5 },
        ErrorCount: { IntegerValue: 1 },
        DanceSummary: { StringValue: 'loaded with errors' },
        LinksCreated: { IntegerValue: 0 },
        LoadCommitStatus: { StringValue: 'CommittedWithWarnings' },
      },
      [
        {
          Filename: { StringValue: 'sample-loader-file.json' },
          StartUtf8ByteOffset: { IntegerValue: 123 },
          LoaderHolonKey: { StringValue: 'alpha' },
          ErrorType: { StringValue: 'ValidationError' },
          ErrorMessage: { StringValue: 'Missing required field' },
        },
      ],
    );

    await expect(presentLoaderResult(loaderHolon)).resolves.toEqual({
      holonsStaged: '8',
      holonsCommitted: '5',
      errorCount: '1',
      danceSummary: 'loaded with errors',
      linksCreated: '0',
      loadCommitStatus: 'CommittedWithWarnings',
      loadErrors: [
        {
          filename: 'sample-loader-file.json',
          startUtf8ByteOffset: '123',
          loaderHolonKey: 'alpha',
          errorType: 'ValidationError',
          errorMessage: 'Missing required field',
        },
      ],
    });
  });
});
