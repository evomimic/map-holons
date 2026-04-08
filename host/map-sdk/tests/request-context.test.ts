import { beforeEach, describe, expect, it } from 'vitest';

import {
  buildRequest,
  defaultRequestOptions,
  nextRequestId,
  resetRequestIdCounter,
} from '../src/internal/request-context';
import type { MapCommandWire } from '../src/internal/wire-types';

// ===========================================
// Request Context Tests
// ===========================================

const beginTransactionCommand: MapCommandWire = {
  Space: 'BeginTransaction',
};

describe('request context', () => {
  beforeEach(() => {
    resetRequestIdCounter();
  });

  it('starts request ids at 1 and increments monotonically', () => {
    expect(nextRequestId()).toBe(1);
    expect(nextRequestId()).toBe(2);
    expect(nextRequestId()).toBe(3);
  });

  it('resets the request counter for test isolation', () => {
    nextRequestId();
    nextRequestId();

    resetRequestIdCounter();

    expect(nextRequestId()).toBe(1);
  });

  it('returns the required default request options', () => {
    expect(defaultRequestOptions()).toEqual({
      gesture_id: null,
      gesture_label: null,
      snapshot_after: false,
    });
  });

  it('returns a fresh default options object each time', () => {
    const first = defaultRequestOptions();
    const second = defaultRequestOptions();

    expect(first).not.toBe(second);
    expect(second).toEqual(first);
  });

  it('builds a request with a generated request id and default options', () => {
    const request = buildRequest(beginTransactionCommand);

    expect(request).toEqual({
      request_id: 1,
      command: beginTransactionCommand,
      options: {
        gesture_id: null,
        gesture_label: null,
        snapshot_after: false,
      },
    });
  });

  it('merges caller-provided request options over the defaults', () => {
    const request = buildRequest(beginTransactionCommand, {
      gesture_id: 'gesture-123',
      gesture_label: 'rename holon',
      snapshot_after: true,
    });

    expect(request).toEqual({
      request_id: 1,
      command: beginTransactionCommand,
      options: {
        gesture_id: 'gesture-123',
        gesture_label: 'rename holon',
        snapshot_after: true,
      },
    });
  });

  it('supports partial request option overrides', () => {
    const request = buildRequest(beginTransactionCommand, {
      gesture_label: 'partial override',
    });

    expect(request.options).toEqual({
      gesture_id: null,
      gesture_label: 'partial override',
      snapshot_after: false,
    });
  });

  it('increments request ids across successive buildRequest calls', () => {
    const first = buildRequest(beginTransactionCommand);
    const second = buildRequest(beginTransactionCommand);

    expect(first.request_id).toBe(1);
    expect(second.request_id).toBe(2);
  });
});
