import { describe, expect, it } from 'vitest';

import * as sdk from '../../src';
import type {
  BaseValue,
  EssentialHolonContent,
  HolonError,
  HolonId,
  LocalId,
  PropertyName,
  ReadableHolon,
  RelationshipName,
  SmartReference,
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
  });

  it('supports the documented public type exports at compile time', () => {
    const baseValue: BaseValue = { StringValue: 'alpha' };
    const holonId: HolonId = { Local: [1, 2, 3] };
    const localId: LocalId = [4, 5, 6];
    const propertyName: PropertyName = 'title';
    const relationshipName: RelationshipName = 'related_to';
    const smartReference: SmartReference = { holonId };
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
    expect(holonId).toEqual({ Local: [1, 2, 3] });
    expect(localId).toEqual([4, 5, 6]);
    expect(propertyName).toBe('title');
    expect(relationshipName).toBe('related_to');
    expect(smartReference).toEqual({ holonId });
    expect(essentialContent.errors).toEqual([holonError]);
  });
});
