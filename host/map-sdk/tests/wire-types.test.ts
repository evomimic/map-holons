import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import {
  isContentSet,
  isFileData,
  isMapIpcRequest,
  isMapIpcResponse,
  isQueryRequestWire,
  isQueryResultWire,
  isRow,
  isRowSet,
  isTransactionActionWire,
} from '../src/internal/wire-types/index';

const fixturesDir = join(dirname(fileURLToPath(import.meta.url)), 'fixtures');
const fixtureFiles = readdirSync(fixturesDir).sort();

describe('wire type fixtures', () => {
  it('discovers the generated fixture set', () => {
    expect(fixtureFiles.length).toBe(42);
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

describe('shared operand wire type guards', () => {
  it('accepts Value-backed row shapes', () => {
    expect(
      isRow({
        title: { StringValue: 'alpha' },
        rank: { IntegerValue: 7 },
      }),
    ).toBe(true);
  });

  it('accepts ordered rowset shapes', () => {
    expect(
      isRowSet({
        rows: [
          { title: { StringValue: 'alpha' } },
          { published: { BooleanValue: true } },
        ],
      }),
    ).toBe(true);
  });

  it('rejects conflated scalar, row, and rowset shapes', () => {
    expect(isRow({ StringValue: 'alpha' })).toBe(false);
    expect(isRowSet({ title: { StringValue: 'alpha' } })).toBe(false);
    expect(
      isRow({
        nested: {
          title: { StringValue: 'alpha' },
        },
      }),
    ).toBe(false);
  });
});

describe('query contract wire type guards', () => {
  it('accepts substrate-facing query request shapes', () => {
    expect(
      isQueryRequestWire({
        target_refs: [],
        query: {
          LegacyRelationshipTraversal: {
            relationship_name: 'children',
          },
        },
        parameters: {
          status: { StringValue: 'Active' },
        },
      }),
    ).toBe(true);
  });

  it('accepts materialized query result envelope shapes', () => {
    expect(
      isQueryResultWire({
        data: {
          RowSet: {
            rows: [{ title: { StringValue: 'alpha' } }],
          },
        },
        diagnostics: [{ code: 'ok', message: 'shape stabilized' }],
      }),
    ).toBe(true);
  });

  it('rejects malformed query contract shapes', () => {
    expect(
      isQueryRequestWire({
        target_refs: [],
        query: {
          relationship_name: 'children',
        },
        parameters: null,
      }),
    ).toBe(false);
    expect(
      isQueryResultWire({
        data: {
          RowSet: { title: { StringValue: 'alpha' } },
        },
        diagnostics: [],
      }),
    ).toBe(false);
  });
});
