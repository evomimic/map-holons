import type { MapTransaction } from '../deps';

const transactionQueues = new WeakMap<MapTransaction, Promise<unknown>>();

/** Orders complete UI operations sharing one stateful transaction.
 * Work inside the queue must not await another operation on this same queue.
 */
export function serializeTransaction<T>(transaction: MapTransaction, work: () => Promise<T>): Promise<T> {
  const previous = transactionQueues.get(transaction) ?? Promise.resolve();
  const next = previous.then(work);
  transactionQueues.set(transaction, next.catch(() => {}));
  return next;
}
