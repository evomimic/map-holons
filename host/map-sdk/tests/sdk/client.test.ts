import { beforeEach, describe, expect, it, vi } from 'vitest';

const { beginTransactionMock } = vi.hoisted(() => ({
  beginTransactionMock: vi.fn(),
}));

vi.mock('../../src/internal/commands/space', () => ({
  beginTransaction: beginTransactionMock,
}));

import { MapClient } from '../../src/sdk/client';
import { MapTransaction } from '../../src/sdk/transaction';

// ===========================================
// MapClient Tests
// ===========================================

describe('MapClient', () => {
  beforeEach(() => {
    beginTransactionMock.mockReset();
  });

  it('delegates beginTransaction and wraps the returned tx id', async () => {
    beginTransactionMock.mockResolvedValue(41);

    const client = new MapClient();
    const transaction = await client.beginTransaction();

    expect(beginTransactionMock).toHaveBeenCalledTimes(1);
    expect(beginTransactionMock).toHaveBeenCalledWith();
    expect(transaction).toBeInstanceOf(MapTransaction);
    expect('_txId' in (transaction as object)).toBe(false);
  });
});
