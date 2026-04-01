import type { RequestOptionsOverrides } from '../request-context';
import { buildRequest } from '../request-context';
import { expectTransactionCreated } from '../result-decoders';
import { invokeMapCommand } from '../transport';
import type { TxId } from '../wire-types';

// ===========================================
// Space Command Builders
// ===========================================

/**
 * Begin a new transaction within the bound MAP space.
 */
export async function beginTransaction(
  options?: RequestOptionsOverrides,
): Promise<TxId> {
  const request = buildRequest(
    {
      Space: 'BeginTransaction',
    },
    options,
  );

  const result = await invokeMapCommand(request);
  return expectTransactionCreated(result);
}
