import { beforeEach, describe, expect, it, vi } from 'vitest';

import { DomainError } from '../../src/internal/errors';
import type {
  BaseValue,
  ContentSet,
  HolonCollectionWire,
  HolonId,
  HolonReferenceWire,
  LocalId,
} from '../../src/internal/wire-types';

const {
  commitMock,
  deleteHolonMock,
  getAllHolonsMock,
  getStagedHolonByBaseKeyMock,
  getStagedHolonsByBaseKeyMock,
  getStagedHolonByVersionedKeyMock,
  getTransientHolonByBaseKeyMock,
  getTransientHolonByVersionedKeyMock,
  loadHolonsMock,
  newHolonMock,
  stagedCountMock,
  stageNewFromCloneMock,
  stageNewHolonMock,
  stageNewVersionFromIdMock,
  stageNewVersionMock,
  transientCountMock,
} = vi.hoisted(() => ({
  commitMock: vi.fn(),
  deleteHolonMock: vi.fn(),
  getAllHolonsMock: vi.fn(),
  getStagedHolonByBaseKeyMock: vi.fn(),
  getStagedHolonsByBaseKeyMock: vi.fn(),
  getStagedHolonByVersionedKeyMock: vi.fn(),
  getTransientHolonByBaseKeyMock: vi.fn(),
  getTransientHolonByVersionedKeyMock: vi.fn(),
  loadHolonsMock: vi.fn(),
  newHolonMock: vi.fn(),
  stagedCountMock: vi.fn(),
  stageNewFromCloneMock: vi.fn(),
  stageNewHolonMock: vi.fn(),
  stageNewVersionFromIdMock: vi.fn(),
  stageNewVersionMock: vi.fn(),
  transientCountMock: vi.fn(),
}));

vi.mock('../../src/internal/commands/transaction', () => ({
  commit: commitMock,
  deleteHolon: deleteHolonMock,
  getAllHolons: getAllHolonsMock,
  getStagedHolonByBaseKey: getStagedHolonByBaseKeyMock,
  getStagedHolonsByBaseKey: getStagedHolonsByBaseKeyMock,
  getStagedHolonByVersionedKey: getStagedHolonByVersionedKeyMock,
  getTransientHolonByBaseKey: getTransientHolonByBaseKeyMock,
  getTransientHolonByVersionedKey: getTransientHolonByVersionedKeyMock,
  loadHolons: loadHolonsMock,
  newHolon: newHolonMock,
  stagedCount: stagedCountMock,
  stageNewFromClone: stageNewFromCloneMock,
  stageNewHolon: stageNewHolonMock,
  stageNewVersion: stageNewVersionMock,
  stageNewVersionFromId: stageNewVersionFromIdMock,
  transientCount: transientCountMock,
}));

import { HolonCollection } from '../../src/sdk/collection';
import {
  createHolonReference,
  createTransientHolonReference,
  HolonReference,
  TransientHolonReference,
} from '../../src/sdk/references';
import { createMapTransaction, MapTransaction } from '../../src/sdk/transaction';

// ===========================================
// MapTransaction Fixtures
// ===========================================

const txId = 41;

const contentSet: ContentSet = {
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

const holonCollection: HolonCollectionWire = {
  state: 'Staged',
  members: [transientReference, stagedReference],
  keyed_index: {
    alpha: 0,
    beta: 1,
  },
};

const integerValue: BaseValue = {
  IntegerValue: 7,
};

const holonId: HolonId = {
  Local: [4, 3, 2, 1],
};

const localId: LocalId = [9, 8, 7];

function transaction(): MapTransaction {
  return createMapTransaction(txId);
}

function transientHandle(): TransientHolonReference {
  return createTransientHolonReference(txId, transientReference);
}

function stagedHandle(): HolonReference {
  return createHolonReference(txId, stagedReference);
}

// ===========================================
// MapTransaction Tests
// ===========================================

describe('MapTransaction', () => {
  beforeEach(() => {
    commitMock.mockReset();
    deleteHolonMock.mockReset();
    getAllHolonsMock.mockReset();
    getStagedHolonByBaseKeyMock.mockReset();
    getStagedHolonsByBaseKeyMock.mockReset();
    getStagedHolonByVersionedKeyMock.mockReset();
    getTransientHolonByBaseKeyMock.mockReset();
    getTransientHolonByVersionedKeyMock.mockReset();
    loadHolonsMock.mockReset();
    newHolonMock.mockReset();
    stagedCountMock.mockReset();
    stageNewFromCloneMock.mockReset();
    stageNewHolonMock.mockReset();
    stageNewVersionFromIdMock.mockReset();
    stageNewVersionMock.mockReset();
    transientCountMock.mockReset();
  });

  it('delegates commit and discards the internal payload', async () => {
    commitMock.mockResolvedValue(transientReference);

    await expect(transaction().commit()).resolves.toBeUndefined();
    expect(commitMock).toHaveBeenCalledTimes(1);
    expect(commitMock).toHaveBeenCalledWith(txId);
  });

  it('wraps newHolon results as transient references', async () => {
    newHolonMock.mockResolvedValue(transientReference);

    const holon = await transaction().newHolon('alpha');

    expect(newHolonMock).toHaveBeenCalledWith(txId, 'alpha');
    expect(holon).toBeInstanceOf(TransientHolonReference);
  });

  it('extracts the transient wire reference for stageNewHolon', async () => {
    stageNewHolonMock.mockResolvedValue(stagedReference);

    const holon = await transaction().stageNewHolon(transientHandle());

    expect(stageNewHolonMock).toHaveBeenCalledWith(txId, transientReference.Transient);
    expect(holon).toBeInstanceOf(HolonReference);
    expect(holon).not.toBeInstanceOf(TransientHolonReference);
  });

  it('delegates stageNewFromClone with the original wire reference', async () => {
    stageNewFromCloneMock.mockResolvedValue(stagedReference);

    const holon = await transaction().stageNewFromClone(stagedHandle(), 'beta');

    expect(stageNewFromCloneMock).toHaveBeenCalledWith(
      txId,
      stagedReference,
      'beta',
    );
    expect(holon).toBeInstanceOf(HolonReference);
  });

  it('converts SmartReference into the expected wire shape', async () => {
    stageNewVersionMock.mockResolvedValue(stagedReference);

    const holon = await transaction().stageNewVersion({
      holonId,
      smartPropertyValues: {
        title: {
          StringValue: 'alpha',
        },
      },
    });

    expect(stageNewVersionMock).toHaveBeenCalledWith(txId, {
      tx_id: txId,
      holon_id: holonId,
      smart_property_values: {
        title: {
          StringValue: 'alpha',
        },
      },
    });
    expect(holon).toBeInstanceOf(HolonReference);
  });

  it('defaults SmartReference.smartPropertyValues to null', async () => {
    stageNewVersionMock.mockResolvedValue(stagedReference);

    await transaction().stageNewVersion({
      holonId,
    });

    expect(stageNewVersionMock).toHaveBeenCalledWith(txId, {
      tx_id: txId,
      holon_id: holonId,
      smart_property_values: null,
    });
  });

  it('delegates stageNewVersionFromId directly', async () => {
    stageNewVersionFromIdMock.mockResolvedValue(stagedReference);

    const holon = await transaction().stageNewVersionFromId(holonId);

    expect(stageNewVersionFromIdMock).toHaveBeenCalledWith(txId, holonId);
    expect(holon).toBeInstanceOf(HolonReference);
  });

  it('delegates deleteHolon directly', async () => {
    deleteHolonMock.mockResolvedValue(undefined);

    await expect(transaction().deleteHolon(localId)).resolves.toBeUndefined();
    expect(deleteHolonMock).toHaveBeenCalledWith(txId, localId);
  });

  it('delegates loadHolons and discards the internal payload', async () => {
    loadHolonsMock.mockResolvedValue(transientReference);

    await expect(transaction().loadHolons(contentSet)).resolves.toBeUndefined();
    expect(loadHolonsMock).toHaveBeenCalledWith(txId, contentSet);
  });

  it('wraps getAllHolons results as a HolonCollection', async () => {
    getAllHolonsMock.mockResolvedValue(holonCollection);

    const collection = await transaction().getAllHolons();

    expect(getAllHolonsMock).toHaveBeenCalledWith(txId);
    expect(collection).toBeInstanceOf(HolonCollection);
    expect(collection.members[0]).toBeInstanceOf(TransientHolonReference);
  });

  it('wraps getStagedHolonsByBaseKey results as public references', async () => {
    getStagedHolonsByBaseKeyMock.mockResolvedValue([
      stagedReference,
      transientReference,
    ]);

    const holons = await transaction().getStagedHolonsByBaseKey('alpha');

    expect(getStagedHolonsByBaseKeyMock).toHaveBeenCalledWith(txId, 'alpha');
    expect(holons).toHaveLength(2);
    expect(holons[0]).toBeInstanceOf(HolonReference);
    expect(holons[1]).toBeInstanceOf(TransientHolonReference);
  });

  it('extracts stagedCount and transientCount integer payloads', async () => {
    stagedCountMock.mockResolvedValue(integerValue);
    transientCountMock.mockResolvedValue(integerValue);

    await expect(transaction().stagedCount()).resolves.toBe(7);
    await expect(transaction().transientCount()).resolves.toBe(7);
    expect(stagedCountMock).toHaveBeenCalledWith(txId);
    expect(transientCountMock).toHaveBeenCalledWith(txId);
  });

  it.each([
    {
      name: 'getStagedHolonByBaseKey',
      run: () => transaction().getStagedHolonByBaseKey('alpha'),
      mock: getStagedHolonByBaseKeyMock,
      expectedClass: HolonReference,
    },
    {
      name: 'getStagedHolonByVersionedKey',
      run: () => transaction().getStagedHolonByVersionedKey('alpha@1'),
      mock: getStagedHolonByVersionedKeyMock,
      expectedClass: HolonReference,
    },
    {
      name: 'getTransientHolonByBaseKey',
      run: () => transaction().getTransientHolonByBaseKey('alpha'),
      mock: getTransientHolonByBaseKeyMock,
      expectedClass: TransientHolonReference,
    },
    {
      name: 'getTransientHolonByVersionedKey',
      run: () => transaction().getTransientHolonByVersionedKey('alpha@1'),
      mock: getTransientHolonByVersionedKeyMock,
      expectedClass: TransientHolonReference,
    },
  ])('$name wraps successful lookup results', async ({ run, mock, expectedClass }) => {
    mock.mockResolvedValue(transientReference);

    const holon = await run();

    expect(mock).toHaveBeenCalledTimes(1);
    expect(holon).toBeInstanceOf(expectedClass);
  });

  it.each([
    {
      name: 'getStagedHolonByBaseKey',
      run: () => transaction().getStagedHolonByBaseKey('alpha'),
      mock: getStagedHolonByBaseKeyMock,
    },
    {
      name: 'getStagedHolonByVersionedKey',
      run: () => transaction().getStagedHolonByVersionedKey('alpha@1'),
      mock: getStagedHolonByVersionedKeyMock,
    },
    {
      name: 'getTransientHolonByBaseKey',
      run: () => transaction().getTransientHolonByBaseKey('alpha'),
      mock: getTransientHolonByBaseKeyMock,
    },
    {
      name: 'getTransientHolonByVersionedKey',
      run: () => transaction().getTransientHolonByVersionedKey('alpha@1'),
      mock: getTransientHolonByVersionedKeyMock,
    },
  ])('$name normalizes HolonNotFound into null', async ({ run, mock }) => {
    mock.mockRejectedValue(new DomainError('HolonNotFound', 'missing-holon'));

    await expect(run()).resolves.toBeNull();
  });

  it.each([
    {
      name: 'getStagedHolonByBaseKey',
      run: () => transaction().getStagedHolonByBaseKey('alpha'),
      mock: getStagedHolonByBaseKeyMock,
    },
    {
      name: 'getStagedHolonByVersionedKey',
      run: () => transaction().getStagedHolonByVersionedKey('alpha@1'),
      mock: getStagedHolonByVersionedKeyMock,
    },
    {
      name: 'getTransientHolonByBaseKey',
      run: () => transaction().getTransientHolonByBaseKey('alpha'),
      mock: getTransientHolonByBaseKeyMock,
    },
    {
      name: 'getTransientHolonByVersionedKey',
      run: () => transaction().getTransientHolonByVersionedKey('alpha@1'),
      mock: getTransientHolonByVersionedKeyMock,
    },
  ])('$name preserves non-normalized domain errors', async ({ run, mock }) => {
    const error = new DomainError('TransactionNotOpen', { tx_id: txId });
    mock.mockRejectedValue(error);

    await expect(run()).rejects.toBe(error);
  });
});
