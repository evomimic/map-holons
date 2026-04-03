import { beforeEach, describe, expect, it, vi } from 'vitest';

import { DomainError, MalformedResponseError, TransportError } from '../src/internal/errors';
import { invokeMapCommand, unwrapMapResponse } from '../src/internal/transport';
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

  it('calls dispatch_map_command and returns the validated response', async () => {
    invokeMock.mockResolvedValue(okResponse);

    await expect(invokeMapCommand(request)).resolves.toEqual(okResponse);
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith('dispatch_map_command', { request });
  });

  it('throws TransportError when the Tauri invoke call rejects', async () => {
    const cause = new Error('plugin unavailable');
    invokeMock.mockRejectedValue(cause);

    await expect(invokeMapCommand(request)).rejects.toMatchObject({
      code: 'TRANSPORT_ERROR',
      cause,
    });
  });

  it('throws MalformedResponseError when response has no request_id', async () => {
    invokeMock.mockResolvedValue({ result: { Ok: okResult } });

    await expect(invokeMapCommand(request)).rejects.toMatchObject({
      code: 'MALFORMED_RESPONSE',
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
});

describe('unwrapMapResponse', () => {
  it('returns the Ok result payload', () => {
    expect(unwrapMapResponse(okResponse)).toEqual(okResult);
  });

  it('throws DomainError when response contains an Err result', () => {
    const errResponse: MapIpcResponse = {
      request_id: 17,
      result: {
        Err: {
          TransactionNotOpen: {
            tx_id: 41,
            state: 'Committed',
          },
        },
      },
    };

    expect(() => unwrapMapResponse(errResponse)).toThrow(
      expect.objectContaining({
        code: 'DOMAIN_ERROR',
        variant: 'TransactionNotOpen',
        payload: {
          tx_id: 41,
          state: 'Committed',
        },
      }),
    );
  });

  it('throws MalformedResponseError when the result envelope is malformed', () => {
    const badResponse: MapIpcResponse = {
      request_id: 17,
      result: {} as MapIpcResponse['result'],
    };

    expect(() => unwrapMapResponse(badResponse)).toThrow(
      expect.objectContaining({
        code: 'MALFORMED_RESPONSE',
      }),
    );
  });
});
