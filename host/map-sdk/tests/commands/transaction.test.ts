import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  commit,
  dance,
  deleteHolon,
  getAllHolons,
  getStagedHolonByBaseKey,
  getStagedHolonByVersionedKey,
  getStagedHolonsByBaseKey,
  getTransientHolonByBaseKey,
  getTransientHolonByVersionedKey,
  loadHolons,
  newHolon,
  query,
  stageNewFromClone,
  stageNewHolon,
  stageNewVersion,
  stageNewVersionFromId,
  stagedCount,
  transientCount,
} from '../../src/internal/commands/transaction';
import { MalformedResponseError } from '../../src/internal/errors';
import { resetRequestIdCounter } from '../../src/internal/request-context';
import type {
  BaseValue,
  ContentSet,
  DanceRequestWire,
  DanceResponseWire,
  HolonCollectionWire,
  HolonId,
  HolonReferenceWire,
  LocalId,
  MapResultWire,
  NodeCollectionWire,
  QueryExpression,
  RequestOptions,
  SmartReferenceWire,
  TransactionActionWire,
  TransientReferenceWire,
} from '../../src/internal/wire-types';

const { invokeMapCommandMock } = vi.hoisted(() => ({
  invokeMapCommandMock: vi.fn(),
}));

vi.mock('../../src/internal/transport', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../src/internal/transport')>();
  return {
    ...actual,
    invokeMapCommand: invokeMapCommandMock,
  };
});

function okResponse(result: MapResultWire) {
  return { request_id: 1, result: { Ok: result } };
}

// ===========================================
// Transaction Command Builder Fixtures
// ===========================================

const txId = 41;
const defaultOptions: RequestOptions = {
  gesture_id: null,
  gesture_label: null,
  snapshot_after: false,
};

const transientWire: TransientReferenceWire = {
  tx_id: txId,
  id: '2f9dcd83-47ee-482e-8059-28dca43d8a64',
};

const transientReference: HolonReferenceWire = {
  Transient: transientWire,
};

const stagedReference: HolonReferenceWire = {
  Staged: {
    tx_id: txId,
    id: 'fcb56a31-c1cb-4066-b4c3-d185809c2864',
  },
};

const smartWire: SmartReferenceWire = {
  tx_id: txId,
  holon_id: {
    Local: [1, 2, 3, 4],
  },
  smart_property_values: null,
};

const localId: LocalId = [9, 8, 7];

const holonId: HolonId = {
  Local: [4, 3, 2, 1],
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

const danceRequest: DanceRequestWire = {
  dance_name: 'sync',
  dance_type: 'Standalone',
  body: 'None',
};

const danceResponse: DanceResponseWire = {
  status_code: 'Accepted',
  description: 'queued',
  body: {
    HolonReference: transientReference,
  },
  descriptor: stagedReference,
};

const queryExpression: QueryExpression = {
  relationship_name: 'related_to',
};

const nodeCollection: NodeCollectionWire = {
  members: [
    {
      source_holon: stagedReference,
      relationships: null,
    },
  ],
  query_spec: queryExpression,
};

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

function expectTransactionRequest(
  action: TransactionActionWire,
  options: RequestOptions = defaultOptions,
) {
  expect(invokeMapCommandMock).toHaveBeenCalledWith({
    request_id: 1,
    command: {
      Transaction: {
        tx_id: txId,
        action,
      },
    },
    options,
  });
}

interface TransactionCase<T> {
  name: string;
  run: () => Promise<T>;
  action: TransactionActionWire;
  okResult: MapResultWire;
  expected: T;
  wrongResult: MapResultWire;
}

const transactionCases: TransactionCase<unknown>[] = [
  {
    name: 'commit',
    run: () => commit(txId),
    action: 'Commit',
    okResult: { Reference: transientReference },
    expected: transientReference,
    wrongResult: 'None',
  },
  {
    name: 'newHolon',
    run: () => newHolon(txId, 'alpha'),
    action: { NewHolon: { key: 'alpha' } },
    okResult: { Reference: transientReference },
    expected: transientReference,
    wrongResult: 'None',
  },
  {
    name: 'stageNewHolon',
    run: () => stageNewHolon(txId, transientWire),
    action: { StageNewHolon: { source: transientWire } },
    okResult: { Reference: stagedReference },
    expected: stagedReference,
    wrongResult: 'None',
  },
  {
    name: 'stageNewFromClone',
    run: () => stageNewFromClone(txId, stagedReference, 'beta'),
    action: { StageNewFromClone: { original: stagedReference, new_key: 'beta' } },
    okResult: { Reference: stagedReference },
    expected: stagedReference,
    wrongResult: 'None',
  },
  {
    name: 'stageNewVersion',
    run: () => stageNewVersion(txId, smartWire),
    action: { StageNewVersion: { current_version: smartWire } },
    okResult: { Reference: stagedReference },
    expected: stagedReference,
    wrongResult: 'None',
  },
  {
    name: 'stageNewVersionFromId',
    run: () => stageNewVersionFromId(txId, holonId),
    action: { StageNewVersionFromId: { holon_id: holonId } },
    okResult: { Reference: stagedReference },
    expected: stagedReference,
    wrongResult: 'None',
  },
  {
    name: 'deleteHolon',
    run: () => deleteHolon(txId, localId),
    action: { DeleteHolon: { local_id: localId } },
    okResult: 'None',
    expected: undefined,
    wrongResult: { Reference: stagedReference },
  },
  {
    name: 'loadHolons',
    run: () => loadHolons(txId, contentSet),
    action: { LoadHolons: { content_set: contentSet } },
    okResult: { Reference: transientReference },
    expected: transientReference,
    wrongResult: 'None',
  },
  {
    name: 'getAllHolons',
    run: () => getAllHolons(txId),
    action: 'GetAllHolons',
    okResult: { Collection: holonCollection },
    expected: holonCollection,
    wrongResult: { References: [stagedReference] },
  },
  {
    name: 'getStagedHolonByBaseKey',
    run: () => getStagedHolonByBaseKey(txId, 'alpha'),
    action: { GetStagedHolonByBaseKey: { key: 'alpha' } },
    okResult: { Reference: stagedReference },
    expected: stagedReference,
    wrongResult: 'None',
  },
  {
    name: 'getStagedHolonsByBaseKey',
    run: () => getStagedHolonsByBaseKey(txId, 'alpha'),
    action: { GetStagedHolonsByBaseKey: { key: 'alpha' } },
    okResult: { References: [stagedReference] },
    expected: [stagedReference],
    wrongResult: { Reference: stagedReference },
  },
  {
    name: 'getStagedHolonByVersionedKey',
    run: () => getStagedHolonByVersionedKey(txId, 'alpha@1'),
    action: { GetStagedHolonByVersionedKey: { key: 'alpha@1' } },
    okResult: { Reference: stagedReference },
    expected: stagedReference,
    wrongResult: 'None',
  },
  {
    name: 'getTransientHolonByBaseKey',
    run: () => getTransientHolonByBaseKey(txId, 'alpha'),
    action: { GetTransientHolonByBaseKey: { key: 'alpha' } },
    okResult: { Reference: transientReference },
    expected: transientReference,
    wrongResult: 'None',
  },
  {
    name: 'getTransientHolonByVersionedKey',
    run: () => getTransientHolonByVersionedKey(txId, 'alpha@1'),
    action: { GetTransientHolonByVersionedKey: { key: 'alpha@1' } },
    okResult: { Reference: transientReference },
    expected: transientReference,
    wrongResult: 'None',
  },
  {
    name: 'stagedCount',
    run: () => stagedCount(txId),
    action: 'StagedCount',
    okResult: { Value: integerValue },
    expected: integerValue,
    wrongResult: 'None',
  },
  {
    name: 'transientCount',
    run: () => transientCount(txId),
    action: 'TransientCount',
    okResult: { Value: integerValue },
    expected: integerValue,
    wrongResult: 'None',
  },
  {
    name: 'dance',
    run: () => dance(txId, danceRequest),
    action: { Dance: danceRequest },
    okResult: { DanceResponse: danceResponse },
    expected: danceResponse,
    wrongResult: { Value: integerValue },
  },
  {
    name: 'query',
    run: () => query(txId, queryExpression),
    action: { Query: queryExpression },
    okResult: { NodeCollection: nodeCollection },
    expected: nodeCollection,
    wrongResult: { Collection: holonCollection },
  },
];

// ===========================================
// Transaction Command Builder Tests
// ===========================================

describe('transaction command builders', () => {
  beforeEach(() => {
    invokeMapCommandMock.mockReset();
    resetRequestIdCounter();
  });

  it.each(transactionCases)(
    'builds $name commands and decodes the expected result',
    async ({ run, action, okResult, expected }) => {
      invokeMapCommandMock.mockResolvedValue(okResponse(okResult));

      await expect(run()).resolves.toEqual(expected);
      expectTransactionRequest(action);
    },
  );

  it.each(transactionCases)(
    'throws MalformedResponseError for $name when the result variant is wrong',
    async ({ run, wrongResult }) => {
      invokeMapCommandMock.mockResolvedValue(okResponse(wrongResult));

      await expect(run()).rejects.toBeInstanceOf(MalformedResponseError);
    },
  );

  it('defaults newHolon key to null when the caller omits it', async () => {
    invokeMapCommandMock.mockResolvedValue(okResponse({ Reference: transientReference }));

    await newHolon(txId);

    expectTransactionRequest({
      NewHolon: {
        key: null,
      },
    });
  });

  it('passes request option overrides through transaction builders', async () => {
    invokeMapCommandMock.mockResolvedValue(okResponse({ Reference: transientReference }));

    await commit(txId, {
      gesture_id: 'gesture-123',
      gesture_label: 'commit',
      snapshot_after: true,
    });

    expectTransactionRequest('Commit', {
      gesture_id: 'gesture-123',
      gesture_label: 'commit',
      snapshot_after: true,
    });
  });
});
