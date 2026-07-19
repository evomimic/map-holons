import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import {
  isContentSet,
  isFileData,
  isMapIpcRequest,
  isMapIpcResponse,
  isPvlFieldWire,
  isPvlMalformedReasonWire,
  isPvlViolationWire,
  isRequestOptions,
  isTransactionActionWire,
} from '../src/internal/wire-types/index';

const fixturesDir = join(dirname(fileURLToPath(import.meta.url)), 'fixtures');
const fixtureFiles = readdirSync(fixturesDir).sort();

describe('wire type fixtures', () => {
  it('discovers the generated fixture set', () => {
    expect(fixtureFiles.length).toBe(39);
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

describe('PVL wire type guards', () => {
  it('accepts every materially distinct violation payload shape', () => {
    const violations = [
      'EmptyPropertyName',
      {
        MalformedSmartLink: {
          reason: { MissingField: 'RelationshipName' },
        },
      },
      {
        UnsupportedNativeValue: {
          property_name: 'status',
          value_kind: 'Map',
        },
      },
      {
        UnsupportedNativeValue: {
          property_name: null,
          value_kind: 'Map',
        },
      },
      {
        StringValueTooLarge: {
          property_name: 'title',
          actual_bytes: 20_000,
          max_bytes: 16_384,
        },
      },
      {
        CanonicalKeyTooLarge: {
          actual_bytes: 300,
          max_bytes: 256,
        },
      },
      {
        ValueNestingTooDeep: {
          property_name: 'items',
          actual_depth: 3,
          max_depth: 2,
        },
      },
      {
        IdentifierTooLong: {
          field_name: 'local_id',
          identifier_kind: 'LocalId',
          actual_bytes: 300,
          max_bytes: 256,
        },
      },
    ];

    expect(violations.every(isPvlViolationWire)).toBe(true);
  });

  it('accepts the exhaustive malformed-reason forms', () => {
    const reasons = [
      'DecodeFailed',
      'InvalidFieldCombination',
      'NonCanonicalEncoding',
      { MissingField: 'RelationshipName' },
      { InvalidDiscriminant: 'PropertyValueDiscriminant' },
      { InvalidUtf8: 'PropertyName' },
      { InvalidLength: 'PropertySection' },
    ];

    expect(reasons.every(isPvlMalformedReasonWire)).toBe(true);
  });

  it('rejects unknown field tokens and malformed reason payloads', () => {
    expect(isPvlFieldWire('UnknownField')).toBe(false);
    expect(isPvlMalformedReasonWire({ MissingField: {} })).toBe(false);
    expect(
      isPvlViolationWire({
        MalformedSmartLink: {
          reason: { MissingField: 'UnknownField' },
        },
      }),
    ).toBe(false);
    expect(isPvlViolationWire({ UnknownViolation: {} })).toBe(false);
  });
});

describe('request options wire type guard', () => {
  it('accepts disable_undo as part of the canonical request options shape', () => {
    expect(
      isRequestOptions({
        marker_id: null,
        marker_label: 'checkpoint',
        snapshot_after: true,
        disable_undo: false,
      }),
    ).toBe(true);
  });

  it('rejects request options missing disable_undo', () => {
    expect(
      isRequestOptions({
        marker_id: null,
        marker_label: 'checkpoint',
        snapshot_after: true,
      }),
    ).toBe(false);
  });
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
