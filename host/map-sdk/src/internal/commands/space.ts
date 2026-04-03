import type { RequestOptionsOverrides } from '../request-context';
import { buildRequest } from '../request-context';
import { expectTransactionCreated } from '../result-decoders';
import { invokeMapCommand, unwrapMapResponse } from '../transport';
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

  const response = await invokeMapCommand(request);
  const result = unwrapMapResponse(response);
  return expectTransactionCreated(result);
}
