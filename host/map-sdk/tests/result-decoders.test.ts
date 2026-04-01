import { describe, expect, it } from 'vitest';

import { MalformedResponseError } from '../src/internal/errors';
import {
  expectCollection,
  expectDanceResponse,
  expectEssentialContent,
  expectHolonId,
  expectNodeCollection,
  expectNone,
  expectOptionalReference,
  expectOptionalValue,
  expectReference,
  expectReferences,
  expectTransactionCreated,
  expectValue,
} from '../src/internal/result-decoders';
import type {
  BaseValue,
  DanceResponseWire,
  EssentialHolonContent,
  HolonCollectionWire,
  HolonId,
  HolonReferenceWire,
  MapResultWire,
  NodeCollectionWire,
} from '../src/internal/wire-types';

// ===========================================
// Result Decoder Fixtures
// ===========================================

const transientReference: HolonReferenceWire = {
  Transient: {
    tx_id: 41,
    id: '2f9dcd83-47ee-482e-8059-28dca43d8a64',
  },
};

const stagedReference: HolonReferenceWire = {
  Staged: {
    tx_id: 41,
    id: 'fcb56a31-c1cb-4066-b4c3-d185809c2864',
  },
};

const baseValue: BaseValue = {
  StringValue: 'alpha',
};

const integerValue: BaseValue = {
  IntegerValue: 7,
};

const holonCollection: HolonCollectionWire = {
  state: 'Staged',
  members: [transientReference, stagedReference],
  keyed_index: {
    alpha: 0,
    beta: 1,
  },
};

const nodeCollection: NodeCollectionWire = {
  members: [
    {
      source_holon: stagedReference,
      relationships: null,
    },
  ],
  query_spec: {
    relationship_name: 'related_to',
  },
};

const holonId: HolonId = {
  Local: [1, 2, 3, 4],
};

const essentialContent: EssentialHolonContent = {
  property_map: {
    title: baseValue,
  },
  key: 'alpha',
  errors: [],
};

const danceResponse: DanceResponseWire = {
  status_code: 'Accepted',
  description: 'dance accepted',
  body: {
    HolonReference: transientReference,
  },
  descriptor: stagedReference,
};

// ===========================================
// Result Decoder Tests
// ===========================================

describe('result decoders', () => {
  it('decodes None results', () => {
    expect(expectNone('None')).toBeUndefined();
  });

  it('throws on non-None results when expecting None', () => {
    expect(() => expectNone({ Reference: transientReference })).toThrow(
      MalformedResponseError,
    );
  });

  it('decodes TransactionCreated results', () => {
    expect(
      expectTransactionCreated({
        TransactionCreated: {
          tx_id: 41,
        },
      }),
    ).toBe(41);
  });

  it('throws on the wrong result variant when expecting TransactionCreated', () => {
    expect(() => expectTransactionCreated('None')).toThrow(
      MalformedResponseError,
    );
  });

  it('decodes Reference results', () => {
    expect(expectReference({ Reference: transientReference })).toEqual(
      transientReference,
    );
  });

  it('throws on the wrong result variant when expecting Reference', () => {
    expect(() => expectReference('None')).toThrow(MalformedResponseError);
  });

  it('decodes optional Reference results', () => {
    expect(expectOptionalReference({ Reference: transientReference })).toEqual(
      transientReference,
    );
    expect(expectOptionalReference('None')).toBeNull();
  });

  it('throws on the wrong result variant when expecting an optional Reference', () => {
    expect(() => expectOptionalReference({ Value: baseValue })).toThrow(
      MalformedResponseError,
    );
  });

  it('decodes References results', () => {
    expect(
      expectReferences({
        References: [transientReference, stagedReference],
      }),
    ).toEqual([transientReference, stagedReference]);
  });

  it('throws on the wrong result variant when expecting References', () => {
    expect(() => expectReferences({ Reference: transientReference })).toThrow(
      MalformedResponseError,
    );
  });

  it('decodes Collection results', () => {
    expect(
      expectCollection({
        Collection: holonCollection,
      }),
    ).toEqual(holonCollection);
  });

  it('throws on the wrong result variant when expecting Collection', () => {
    expect(() => expectCollection({ References: [transientReference] })).toThrow(
      MalformedResponseError,
    );
  });

  it('decodes NodeCollection results', () => {
    expect(
      expectNodeCollection({
        NodeCollection: nodeCollection,
      }),
    ).toEqual(nodeCollection);
  });

  it('throws on the wrong result variant when expecting NodeCollection', () => {
    expect(() => expectNodeCollection({ Collection: holonCollection })).toThrow(
      MalformedResponseError,
    );
  });

  it('decodes Value results', () => {
    expect(
      expectValue({
        Value: baseValue,
      }),
    ).toEqual(baseValue);
  });

  it('throws on the wrong result variant when expecting Value', () => {
    expect(() => expectValue('None')).toThrow(MalformedResponseError);
  });

  it('decodes optional Value results', () => {
    expect(
      expectOptionalValue({
        Value: integerValue,
      }),
    ).toEqual(integerValue);
    expect(expectOptionalValue('None')).toBeNull();
  });

  it('throws on the wrong result variant when expecting an optional Value', () => {
    expect(() => expectOptionalValue({ Reference: transientReference })).toThrow(
      MalformedResponseError,
    );
  });

  it('decodes HolonId results', () => {
    expect(
      expectHolonId({
        HolonId: holonId,
      }),
    ).toEqual(holonId);
  });

  it('throws on the wrong result variant when expecting HolonId', () => {
    expect(() => expectHolonId({ Value: integerValue })).toThrow(
      MalformedResponseError,
    );
  });

  it('decodes EssentialContent results', () => {
    expect(
      expectEssentialContent({
        EssentialContent: essentialContent,
      }),
    ).toEqual(essentialContent);
  });

  it('throws on the wrong result variant when expecting EssentialContent', () => {
    expect(() => expectEssentialContent({ Reference: stagedReference })).toThrow(
      MalformedResponseError,
    );
  });

  it('decodes DanceResponse results', () => {
    expect(
      expectDanceResponse({
        DanceResponse: danceResponse,
      }),
    ).toEqual(danceResponse);
  });

  it('throws on the wrong result variant when expecting DanceResponse', () => {
    expect(() => expectDanceResponse({ Value: baseValue })).toThrow(
      MalformedResponseError,
    );
  });

  it('includes expected and actual variant details in malformed-result errors', () => {
    let error: unknown;

    try {
      expectTransactionCreated({ Value: baseValue } as MapResultWire);
    } catch (caught) {
      error = caught;
    }

    expect(error).toBeInstanceOf(MalformedResponseError);
    expect(error).toMatchObject({
      code: 'MALFORMED_RESPONSE',
      details: {
        expected: 'TransactionCreated',
        actual: 'Value',
      },
    });
  });
});
