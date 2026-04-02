import * as internalSpace from '../internal/commands/space';
import { MapTransaction } from './transaction';

// ===========================================
// Public MAP Client
// ===========================================

/**
 * Root public SDK entrypoint for transaction-scoped MAP work.
 *
 * Transaction identity stays internal to the transaction object returned by
 * `beginTransaction()`.
 */
export class MapClient {
  /**
   * Begin a new transaction and return the bound public transaction wrapper.
   */
  async beginTransaction(): Promise<MapTransaction> {
    const txId = await internalSpace.beginTransaction();
    return MapTransaction._fromTxId(txId);
  }
}
