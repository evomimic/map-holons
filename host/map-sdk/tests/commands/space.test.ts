import { beforeEach, describe, expect, it, vi } from 'vitest';

import { beginTransaction } from '../../src/internal/commands/space';
import { MalformedResponseError } from '../../src/internal/errors';
import { resetRequestIdCounter } from '../../src/internal/request-context';

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

// ===========================================
// Space Command Builder Tests
// ===========================================

describe('space command builders', () => {
  beforeEach(() => {
    invokeMapCommandMock.mockReset();
    resetRequestIdCounter();
  });

  it('builds a BeginTransaction request and decodes the returned tx id', async () => {
    invokeMapCommandMock.mockResolvedValue({
      request_id: 1,
      result: {
        Ok: {
          TransactionCreated: {
            tx_id: 41,
          },
        },
      },
    });

    await expect(beginTransaction()).resolves.toBe(41);
    expect(invokeMapCommandMock).toHaveBeenCalledWith({
      request_id: 1,
      command: {
        Space: 'BeginTransaction',
      },
      options: {
        gesture_id: null,
        gesture_label: null,
        snapshot_after: false,
      },
    });
  });

  it('passes request option overrides through to the request envelope', async () => {
    invokeMapCommandMock.mockResolvedValue({
      request_id: 1,
      result: {
        Ok: {
          TransactionCreated: {
            tx_id: 42,
          },
        },
      },
    });

    await beginTransaction({
      gesture_id: 'gesture-123',
      gesture_label: 'space-start',
      snapshot_after: true,
    });

    expect(invokeMapCommandMock).toHaveBeenCalledWith({
      request_id: 1,
      command: {
        Space: 'BeginTransaction',
      },
      options: {
        gesture_id: 'gesture-123',
        gesture_label: 'space-start',
        snapshot_after: true,
      },
    });
  });

  it('throws MalformedResponseError when the transport returns the wrong result variant', async () => {
    invokeMapCommandMock.mockResolvedValue({
      request_id: 1,
      result: {
        Ok: 'None',
      },
    });

    await expect(beginTransaction()).rejects.toBeInstanceOf(
      MalformedResponseError,
    );
  });
});
