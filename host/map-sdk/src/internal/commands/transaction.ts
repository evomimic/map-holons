import type { RequestOptionsOverrides } from '../request-context';
import { buildRequest } from '../request-context';
import {
  expectCollection,
  expectDanceResponse,
  expectNodeCollection,
  expectNone,
  expectReference,
  expectReferences,
  expectValue,
} from '../result-decoders';
import { invokeMapCommand, unwrapMapResponse } from '../transport';
import type {
  BaseValue,
  DanceRequestWire,
  DanceResponseWire,
  HolonCollectionWire,
  HolonId,
  HolonReferenceWire,
  LocalId,
  MapResultWire,
  NodeCollectionWire,
  QueryExpression,
  SmartReferenceWire,
  TransactionActionWire,
  TransientReferenceWire,
  TxId,
} from '../wire-types';

// ===========================================
// Transaction Command Builders
// ===========================================

type ResultDecoder<T> = (result: MapResultWire) => T;

function buildTransactionRequest(
  txId: TxId,
  action: TransactionActionWire,
  options?: RequestOptionsOverrides,
) {
  return buildRequest(
    {
      Transaction: {
        tx_id: txId,
        action,
      },
    },
    options,
  );
}

async function runTransactionCommand<T>(
  txId: TxId,
  action: TransactionActionWire,
  decode: ResultDecoder<T>,
  options?: RequestOptionsOverrides,
): Promise<T> {
  const request = buildTransactionRequest(txId, action, options);
  const response = await invokeMapCommand(request);
  const result = unwrapMapResponse(response);
  return decode(result);
}

/**
 * Commit an open transaction and return the runtime response reference.
 *
 * The public SDK currently discards this payload and exposes `Promise<void>`.
 */
export function commit(
  txId: TxId,
  options?: RequestOptionsOverrides,
): Promise<HolonReferenceWire> {
  return runTransactionCommand(txId, 'Commit', expectReference, options);
}

/**
 * Create a new transient holon.
 */
export function newHolon(
  txId: TxId,
  key?: string | null,
  options?: RequestOptionsOverrides,
): Promise<HolonReferenceWire> {
  return runTransactionCommand(
    txId,
    {
      NewHolon: {
        key: key ?? null,
      },
    },
    expectReference,
    options,
  );
}

/**
 * Stage a transient holon as a new staged holon.
 */
export function stageNewHolon(
  txId: TxId,
  source: TransientReferenceWire,
  options?: RequestOptionsOverrides,
): Promise<HolonReferenceWire> {
  return runTransactionCommand(
    txId,
    {
      StageNewHolon: {
        source,
      },
    },
    expectReference,
    options,
  );
}

/**
 * Stage a new holon from an existing clone source.
 */
export function stageNewFromClone(
  txId: TxId,
  original: HolonReferenceWire,
  newKey: string,
  options?: RequestOptionsOverrides,
): Promise<HolonReferenceWire> {
  return runTransactionCommand(
    txId,
    {
      StageNewFromClone: {
        original,
        new_key: newKey,
      },
    },
    expectReference,
    options,
  );
}

/**
 * Stage a new version from a smart reference.
 */
export function stageNewVersion(
  txId: TxId,
  currentVersion: SmartReferenceWire,
  options?: RequestOptionsOverrides,
): Promise<HolonReferenceWire> {
  return runTransactionCommand(
    txId,
    {
      StageNewVersion: {
        current_version: currentVersion,
      },
    },
    expectReference,
    options,
  );
}

/**
 * Stage a new version from a persisted holon id.
 */
export function stageNewVersionFromId(
  txId: TxId,
  holonId: HolonId,
  options?: RequestOptionsOverrides,
): Promise<HolonReferenceWire> {
  return runTransactionCommand(
    txId,
    {
      StageNewVersionFromId: {
        holon_id: holonId,
      },
    },
    expectReference,
    options,
  );
}

/**
 * Delete a local holon in the active transaction.
 */
export function deleteHolon(
  txId: TxId,
  localId: LocalId,
  options?: RequestOptionsOverrides,
): Promise<void> {
  return runTransactionCommand(
    txId,
    {
      DeleteHolon: {
        local_id: localId,
      },
    },
    expectNone,
    options,
  );
}

/**
 * Load a bundle of holons and return the runtime response reference.
 *
 * This remains reference-returning internally because current runtime behavior
 * is terminal or commit-like rather than a pure in-transaction mutation.
 */
export function loadHolons(
  txId: TxId,
  bundle: HolonReferenceWire,
  options?: RequestOptionsOverrides,
): Promise<HolonReferenceWire> {
  return runTransactionCommand(
    txId,
    {
      LoadHolons: {
        bundle,
      },
    },
    expectReference,
    options,
  );
}

/**
 * Return the full holon collection visible to the transaction.
 */
export function getAllHolons(
  txId: TxId,
  options?: RequestOptionsOverrides,
): Promise<HolonCollectionWire> {
  return runTransactionCommand(txId, 'GetAllHolons', expectCollection, options);
}

/**
 * Return the staged holon bound to a base key.
 */
export function getStagedHolonByBaseKey(
  txId: TxId,
  key: string,
  options?: RequestOptionsOverrides,
): Promise<HolonReferenceWire> {
  return runTransactionCommand(
    txId,
    {
      GetStagedHolonByBaseKey: {
        key,
      },
    },
    expectReference,
    options,
  );
}

/**
 * Return all staged holons bound to a base key.
 */
export function getStagedHolonsByBaseKey(
  txId: TxId,
  key: string,
  options?: RequestOptionsOverrides,
): Promise<HolonReferenceWire[]> {
  return runTransactionCommand(
    txId,
    {
      GetStagedHolonsByBaseKey: {
        key,
      },
    },
    expectReferences,
    options,
  );
}

/**
 * Return the staged holon bound to a versioned key.
 */
export function getStagedHolonByVersionedKey(
  txId: TxId,
  key: string,
  options?: RequestOptionsOverrides,
): Promise<HolonReferenceWire> {
  return runTransactionCommand(
    txId,
    {
      GetStagedHolonByVersionedKey: {
        key,
      },
    },
    expectReference,
    options,
  );
}

/**
 * Return the transient holon bound to a base key.
 */
export function getTransientHolonByBaseKey(
  txId: TxId,
  key: string,
  options?: RequestOptionsOverrides,
): Promise<HolonReferenceWire> {
  return runTransactionCommand(
    txId,
    {
      GetTransientHolonByBaseKey: {
        key,
      },
    },
    expectReference,
    options,
  );
}

/**
 * Return the transient holon bound to a versioned key.
 */
export function getTransientHolonByVersionedKey(
  txId: TxId,
  key: string,
  options?: RequestOptionsOverrides,
): Promise<HolonReferenceWire> {
  return runTransactionCommand(
    txId,
    {
      GetTransientHolonByVersionedKey: {
        key,
      },
    },
    expectReference,
    options,
  );
}

/**
 * Return the staged holon count as a wire `BaseValue`.
 */
export function stagedCount(
  txId: TxId,
  options?: RequestOptionsOverrides,
): Promise<BaseValue> {
  return runTransactionCommand(txId, 'StagedCount', expectValue, options);
}

/**
 * Return the transient holon count as a wire `BaseValue`.
 */
export function transientCount(
  txId: TxId,
  options?: RequestOptionsOverrides,
): Promise<BaseValue> {
  return runTransactionCommand(txId, 'TransientCount', expectValue, options);
}

/**
 * Execute a transaction-scoped dance request.
 */
export function dance(
  txId: TxId,
  request: DanceRequestWire,
  options?: RequestOptionsOverrides,
): Promise<DanceResponseWire> {
  return runTransactionCommand(
    txId,
    {
      Dance: request,
    },
    expectDanceResponse,
    options,
  );
}

/**
 * Execute a transaction-scoped query expression.
 */
export function query(
  txId: TxId,
  expression: QueryExpression,
  options?: RequestOptionsOverrides,
): Promise<NodeCollectionWire> {
  return runTransactionCommand(
    txId,
    {
      Query: expression,
    },
    expectNodeCollection,
    options,
  );
}
