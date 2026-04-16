import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import {
  isContentSet,
  isFileData,
  isMapIpcRequest,
  isMapIpcResponse,
  isTransactionActionWire,
} from '../src/internal/wire-types/index';

const fixturesDir = join(dirname(fileURLToPath(import.meta.url)), 'fixtures');
const fixtureFiles = readdirSync(fixturesDir).sort();

describe('wire type fixtures', () => {
  it('discovers the generated fixture set', () => {
    expect(fixtureFiles.length).toBe(41);
  });

  for (const fixtureFile of fixtureFiles) {
    it(`validates ${fixtureFile}`, () => {
      const fixturePath = join(fixturesDir, fixtureFile);
      const payload = JSON.parse(readFileSync(fixturePath, 'utf8')) as unknown;

      const isValid =
        fixtureFile.startsWith('request-')
          ? isMapIpcRequest(payload)
          : isMapIpcResponse(payload);

      expect(isValid).toBe(true);
    });
  }
});

describe('LoadHolons wire type guard', () => {
  const contentSet = {
    schema: {
      filename: 'bootstrap-import.schema.json',
      raw_contents: '{"type":"object"}',
    },
    files_to_load: [
      {
        filename: 'sample-loader-file.json',
        raw_contents: '{"holons":[]}',
      },
    ],
  };

  it('accepts ContentSet payloads', () => {
    expect(isFileData(contentSet.schema)).toBe(true);
    expect(isContentSet(contentSet)).toBe(true);
    expect(isTransactionActionWire({ LoadHolons: { content_set: contentSet } })).toBe(
      true,
    );
  });

  it('rejects the former bundle payload shape', () => {
    expect(
      isTransactionActionWire({
        LoadHolons: {
          bundle: {
            Staged: {
              tx_id: 41,
              id: '22222222-2222-2222-2222-222222222222',
            },
          },
        },
      }),
    ).toBe(false);
  });
});
