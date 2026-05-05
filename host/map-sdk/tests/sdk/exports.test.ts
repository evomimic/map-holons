import { describe, expect, it } from 'vitest';

import * as sdk from '../../src';
import type {
  BaseValue,
  ContentSet,
  EssentialHolonContent,
  FileData,
  HolonError,
  HolonId,
  LocalId,
  PropertyName,
  QueryRequest,
  QueryResult,
  ReadableHolon,
  Row,
  RowSet,
  RelationshipName,
  SmartReference,
  Value,
  WritableHolon,
} from '../../src';

// ===========================================
// Public Export Tests
// ===========================================

describe('public SDK exports', () => {
  it('exports the documented public runtime surface', () => {
    expect(sdk.MapClient).toBeDefined();
    expect(sdk.MapTransaction).toBeDefined();
    expect(sdk.HolonReference).toBeDefined();
    expect(sdk.TransientHolonReference).toBeDefined();
    expect(sdk.HolonCollection).toBeDefined();
    expect(sdk.MapError).toBeDefined();
    expect(sdk.TransportError).toBeDefined();
    expect(sdk.MalformedResponseError).toBeDefined();
    expect(sdk.DomainError).toBeDefined();
    expect(sdk.extractString).toBeDefined();
    expect(sdk.extractNumber).toBeDefined();
  });

  it('does not expose internal wire or transport-layer exports', () => {
    expect('MapIpcRequest' in sdk).toBe(false);
    expect('MapIpcResponse' in sdk).toBe(false);
    expect('RequestOptions' in sdk).toBe(false);
    expect('RequestOptionsOverrides' in sdk).toBe(false);
    expect('TxId' in sdk).toBe(false);
    expect('MapCommandWire' in sdk).toBe(false);
    expect('TransactionActionWire' in sdk).toBe(false);
    expect('HolonActionWire' in sdk).toBe(false);
    expect('MapResultWire' in sdk).toBe(false);
    expect('invokeMapCommand' in sdk).toBe(false);
    expect('dance' in sdk).toBe(false);
    expect('query' in sdk).toBe(false);
    expect('createMapTransaction' in sdk).toBe(false);
    expect('createHolonReference' in sdk).toBe(false);
    expect('createTransientHolonReference' in sdk).toBe(false);
    expect('unwrapHolonReference' in sdk).toBe(false);
    expect('unwrapTransientHolonReference' in sdk).toBe(false);
  });

  it('does not leave dance/query on the public MapTransaction prototype', () => {
    expect('dance' in sdk.MapTransaction.prototype).toBe(false);
    expect('query' in sdk.MapTransaction.prototype).toBe(true);
  });

  it('supports the documented public type exports at compile time', () => {
    const baseValue: BaseValue = { StringValue: 'alpha' };
    const value: Value = { IntegerValue: 7 };
    const holonId: HolonId = { Local: [1, 2, 3] };
    const localId: LocalId = [4, 5, 6];
    const propertyName: PropertyName = 'title';
    const relationshipName: RelationshipName = 'related_to';
    const row: Row = {
      title: baseValue,
      rank: value,
    };
    const rowSet: RowSet = {
      rows: [row, { published: { BooleanValue: true } }],
    };
    const queryRequest: QueryRequest = {
      target_refs: [],
      query: {
        LegacyRelationshipTraversal: {
          relationship_name: relationshipName,
        },
      },
      parameters: row,
    };
    const queryResult: QueryResult = {
      data: {
        RowSet: rowSet,
      },
      diagnostics: [],
    };
    const smartReference: SmartReference = { holonId };
    const fileData: FileData = {
      filename: 'sample-loader-file.json',
      raw_contents: '{"holons":[]}',
    };
    const contentSet: ContentSet = {
      schema: {
        filename: 'bootstrap-import.schema.json',
        raw_contents: '{"type":"object"}',
      },
      files_to_load: [fileData],
    };
    const holonError: HolonError = { HolonNotFound: 'missing-holon' };
    const essentialContent: EssentialHolonContent = {
      property_map: {
        title: baseValue,
      },
      key: 'alpha',
      errors: [holonError],
    };

    const acceptsReadable = (_value: ReadableHolon | null): void => {};
    const acceptsWritable = (_value: WritableHolon | null): void => {};

    acceptsReadable(null);
    acceptsWritable(null);

    expect(baseValue).toEqual({ StringValue: 'alpha' });
    expect(value).toEqual({ IntegerValue: 7 });
    expect(holonId).toEqual({ Local: [1, 2, 3] });
    expect(localId).toEqual([4, 5, 6]);
    expect(propertyName).toBe('title');
    expect(relationshipName).toBe('related_to');
    expect(queryRequest.query.LegacyRelationshipTraversal.relationship_name).toBe(
      'related_to',
    );
    expect(queryResult.data).toEqual({ RowSet: rowSet });
    expect(row['rank']).toEqual({ IntegerValue: 7 });
    expect(rowSet.rows).toHaveLength(2);
    expect(smartReference).toEqual({ holonId });
    expect(contentSet.files_to_load).toEqual([fileData]);
    expect(essentialContent.errors).toEqual([holonError]);
  });
});
