import { beforeEach, describe, expect, it, vi } from 'vitest';

import type {
  BaseValue,
  EssentialHolonContent,
  HolonCollectionWire,
  HolonId,
  HolonReferenceWire,
} from '../../src/internal/wire-types';

const {
  addRelatedHolonsMock,
  cloneHolonMock,
  essentialContentMock,
  predecessorMock,
  readHolonIdMock,
  readKeyMock,
  readPropertyValueMock,
  readRelatedHolonsMock,
  readVersionedKeyMock,
  removePropertyValueMock,
  removeRelatedHolonsMock,
  summarizeMock,
  withDescriptorMock,
  withPropertyValueMock,
} = vi.hoisted(() => ({
  addRelatedHolonsMock: vi.fn(),
  cloneHolonMock: vi.fn(),
  essentialContentMock: vi.fn(),
  predecessorMock: vi.fn(),
  readHolonIdMock: vi.fn(),
  readKeyMock: vi.fn(),
  readPropertyValueMock: vi.fn(),
  readRelatedHolonsMock: vi.fn(),
  readVersionedKeyMock: vi.fn(),
  removePropertyValueMock: vi.fn(),
  removeRelatedHolonsMock: vi.fn(),
  summarizeMock: vi.fn(),
  withDescriptorMock: vi.fn(),
  withPropertyValueMock: vi.fn(),
}));

vi.mock('../../src/internal/commands/holon', () => ({
  addRelatedHolons: addRelatedHolonsMock,
  cloneHolon: cloneHolonMock,
  essentialContent: essentialContentMock,
  predecessor: predecessorMock,
  readHolonId: readHolonIdMock,
  readKey: readKeyMock,
  readPropertyValue: readPropertyValueMock,
  readRelatedHolons: readRelatedHolonsMock,
  readVersionedKey: readVersionedKeyMock,
  removePropertyValue: removePropertyValueMock,
  removeRelatedHolons: removeRelatedHolonsMock,
  summarize: summarizeMock,
  withDescriptor: withDescriptorMock,
  withPropertyValue: withPropertyValueMock,
}));

import { HolonCollection } from '../../src/sdk/collection';
import {
  HolonReference,
  TransientHolonReference,
} from '../../src/sdk/references';

// ===========================================
// Holon Reference Fixtures
// ===========================================

const txId = 41;

const transientReference: HolonReferenceWire = {
  Transient: {
    tx_id: txId,
    id: '2f9dcd83-47ee-482e-8059-28dca43d8a64',
  },
};

const stagedReference: HolonReferenceWire = {
  Staged: {
    tx_id: txId,
    id: 'fcb56a31-c1cb-4066-b4c3-d185809c2864',
  },
};

const smartReference: HolonReferenceWire = {
  Smart: {
    tx_id: txId,
    holon_id: {
      Local: [1, 2, 3, 4],
    },
    smart_property_values: null,
  },
};

const stringValue: BaseValue = {
  StringValue: 'alpha',
};

const propertyValue: BaseValue = {
  EnumValue: 'Draft',
};

const holonId: HolonId = {
  Local: [9, 8, 7, 6],
};

const essentialContent: EssentialHolonContent = {
  property_map: {
    title: stringValue,
  },
  key: 'alpha',
  errors: [],
};

const relatedCollection: HolonCollectionWire = {
  state: 'Fetched',
  members: [transientReference, smartReference],
  keyed_index: {
    alpha: 0,
    beta: 1,
  },
};

function stagedHandle(): HolonReference {
  return HolonReference._fromWire(txId, stagedReference);
}

function transientHandle(): TransientHolonReference {
  return TransientHolonReference._fromWire(txId, transientReference);
}

// ===========================================
// HolonReference Tests
// ===========================================

describe('HolonReference', () => {
  beforeEach(() => {
    addRelatedHolonsMock.mockReset();
    cloneHolonMock.mockReset();
    essentialContentMock.mockReset();
    predecessorMock.mockReset();
    readHolonIdMock.mockReset();
    readKeyMock.mockReset();
    readPropertyValueMock.mockReset();
    readRelatedHolonsMock.mockReset();
    readVersionedKeyMock.mockReset();
    removePropertyValueMock.mockReset();
    removeRelatedHolonsMock.mockReset();
    summarizeMock.mockReset();
    withDescriptorMock.mockReset();
    withPropertyValueMock.mockReset();
  });

  it('wraps transient wire references as TransientHolonReference instances', () => {
    const reference = HolonReference._fromWire(txId, transientReference);

    expect(reference).toBeInstanceOf(TransientHolonReference);
  });

  it('rejects non-transient wire references in the transient-only factory', () => {
    expect(() =>
      TransientHolonReference._fromWire(txId, stagedReference),
    ).toThrow('Expected a transient holon reference');
  });

  it('delegates cloneHolon and wraps the result as a transient reference', async () => {
    cloneHolonMock.mockResolvedValue(transientReference);

    const clone = await stagedHandle().cloneHolon();

    expect(cloneHolonMock).toHaveBeenCalledTimes(1);
    expect(cloneHolonMock).toHaveBeenCalledWith(txId, stagedReference);
    expect(clone).toBeInstanceOf(TransientHolonReference);
  });

  it('delegates essentialContent directly', async () => {
    essentialContentMock.mockResolvedValue(essentialContent);

    await expect(stagedHandle().essentialContent()).resolves.toEqual(
      essentialContent,
    );
    expect(essentialContentMock).toHaveBeenCalledWith(txId, stagedReference);
  });

  it('extracts the summarized string payload', async () => {
    summarizeMock.mockResolvedValue(stringValue);

    await expect(stagedHandle().summarize()).resolves.toBe('alpha');
    expect(summarizeMock).toHaveBeenCalledWith(txId, stagedReference);
  });

  it('delegates holonId directly', async () => {
    readHolonIdMock.mockResolvedValue(holonId);

    await expect(stagedHandle().holonId()).resolves.toEqual(holonId);
    expect(readHolonIdMock).toHaveBeenCalledWith(txId, stagedReference);
  });

  it('wraps predecessor references and preserves null predecessors', async () => {
    predecessorMock.mockResolvedValueOnce(transientReference);
    predecessorMock.mockResolvedValueOnce(null);

    const wrapped = await stagedHandle().predecessor();
    const missing = await stagedHandle().predecessor();

    expect(predecessorMock).toHaveBeenNthCalledWith(1, txId, stagedReference);
    expect(predecessorMock).toHaveBeenNthCalledWith(2, txId, stagedReference);
    expect(wrapped).toBeInstanceOf(TransientHolonReference);
    expect(missing).toBeNull();
  });

  it('extracts key and versionedKey string payloads', async () => {
    readKeyMock.mockResolvedValueOnce(stringValue);
    readKeyMock.mockResolvedValueOnce(null);
    readVersionedKeyMock.mockResolvedValue(stringValue);

    await expect(stagedHandle().key()).resolves.toBe('alpha');
    await expect(stagedHandle().key()).resolves.toBeNull();
    await expect(stagedHandle().versionedKey()).resolves.toBe('alpha');
    expect(readVersionedKeyMock).toHaveBeenCalledWith(txId, stagedReference);
  });

  it('delegates propertyValue directly', async () => {
    readPropertyValueMock.mockResolvedValue(propertyValue);

    await expect(stagedHandle().propertyValue('status')).resolves.toEqual(
      propertyValue,
    );
    expect(readPropertyValueMock).toHaveBeenCalledWith(
      txId,
      stagedReference,
      'status',
    );
  });

  it('wraps relatedHolons as a public HolonCollection', async () => {
    readRelatedHolonsMock.mockResolvedValue(relatedCollection);

    const collection = await stagedHandle().relatedHolons('related_to');

    expect(readRelatedHolonsMock).toHaveBeenCalledWith(
      txId,
      stagedReference,
      'related_to',
    );
    expect(collection).toBeInstanceOf(HolonCollection);
    expect(collection.members[0]).toBeInstanceOf(TransientHolonReference);
    expect(collection.members[1]).toBeInstanceOf(HolonReference);
  });

  it('delegates property mutation methods with the expected arguments', async () => {
    withPropertyValueMock.mockResolvedValue(undefined);
    removePropertyValueMock.mockResolvedValue(undefined);

    await expect(
      stagedHandle().withPropertyValue('title', stringValue),
    ).resolves.toBeUndefined();
    await expect(stagedHandle().removePropertyValue('title')).resolves.toBeUndefined();

    expect(withPropertyValueMock).toHaveBeenCalledWith(
      txId,
      stagedReference,
      'title',
      stringValue,
    );
    expect(removePropertyValueMock).toHaveBeenCalledWith(
      txId,
      stagedReference,
      'title',
    );
  });

  it('extracts wire references for related-holon mutation methods', async () => {
    addRelatedHolonsMock.mockResolvedValue(undefined);
    removeRelatedHolonsMock.mockResolvedValue(undefined);

    const related = [stagedHandle(), transientHandle()];

    await expect(
      stagedHandle().addRelatedHolons('related_to', related),
    ).resolves.toBeUndefined();
    await expect(
      stagedHandle().removeRelatedHolons('related_to', related),
    ).resolves.toBeUndefined();

    expect(addRelatedHolonsMock).toHaveBeenCalledWith(
      txId,
      stagedReference,
      'related_to',
      [stagedReference, transientReference],
    );
    expect(removeRelatedHolonsMock).toHaveBeenCalledWith(
      txId,
      stagedReference,
      'related_to',
      [stagedReference, transientReference],
    );
  });

  it('extracts the descriptor wire reference for withDescriptor', async () => {
    withDescriptorMock.mockResolvedValue(undefined);

    await expect(
      stagedHandle().withDescriptor(HolonReference._fromWire(txId, smartReference)),
    ).resolves.toBeUndefined();

    expect(withDescriptorMock).toHaveBeenCalledWith(
      txId,
      stagedReference,
      smartReference,
    );
  });
});
