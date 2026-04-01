import { beforeEach, describe, expect, it, vi } from 'vitest';

import { DomainError, MalformedResponseError, TransportError } from '../src/internal/errors';
import { invokeMapCommand } from '../src/internal/transport';
import type { MapIpcRequest, MapIpcResponse, MapResultWire } from '../src/internal/wire-types';

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

// ===========================================
// Transport Test Fixtures
// ===========================================

const request: MapIpcRequest = {
  request_id: 17,
  command: {
    Space: 'BeginTransaction',
  },
  options: {
    gesture_id: null,
    gesture_label: null,
    snapshot_after: false,
  },
};

const okResult: MapResultWire = {
  TransactionCreated: {
    tx_id: 41,
  },
};

const okResponse: MapIpcResponse = {
  request_id: request.request_id,
  result: {
    Ok: okResult,
  },
};

describe('invokeMapCommand', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('calls dispatch_map_command and returns the Ok result payload', async () => {
    invokeMock.mockResolvedValue(okResponse);

    await expect(invokeMapCommand(request)).resolves.toEqual(okResult);
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith('dispatch_map_command', { request });
  });

  it('throws DomainError when Rust returns an Err result', async () => {
    invokeMock.mockResolvedValue({
      request_id: request.request_id,
      result: {
        Err: {
          TransactionNotOpen: {
            tx_id: 41,
            state: 'Committed',
          },
        },
      },
    });

    await expect(invokeMapCommand(request)).rejects.toMatchObject({
      code: 'DOMAIN_ERROR',
      variant: 'TransactionNotOpen',
      payload: {
        tx_id: 41,
        state: 'Committed',
      },
    });
  });

  it('throws TransportError when the Tauri invoke call rejects', async () => {
    const cause = new Error('plugin unavailable');
    invokeMock.mockRejectedValue(cause);

    await expect(invokeMapCommand(request)).rejects.toMatchObject({
      code: 'TRANSPORT_ERROR',
      cause,
    });
  });

  it('throws MalformedResponseError on request/response correlation mismatch', async () => {
    invokeMock.mockResolvedValue({
      request_id: request.request_id + 1,
      result: {
        Ok: okResult,
      },
    });

    await expect(invokeMapCommand(request)).rejects.toMatchObject({
      code: 'MALFORMED_RESPONSE',
      details: {
        request_id: request.request_id,
        response_request_id: request.request_id + 1,
      },
    });
  });

  it('throws MalformedResponseError when the result envelope is malformed', async () => {
    invokeMock.mockResolvedValue({
      request_id: request.request_id,
      result: {},
    });

    await expect(invokeMapCommand(request)).rejects.toMatchObject({
      code: 'MALFORMED_RESPONSE',
    });
  });
});
