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
      marker_id: null,
      marker_label: null,
      snapshot_after: false,
      disable_undo: false,
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
        marker_id: null,
        marker_label: null,
        snapshot_after: false,
        disable_undo: false,
      },
    });
  });

  it('merges caller-provided request options over the defaults', () => {
    const request = buildRequest(beginTransactionCommand, {
      marker_id: 'gesture-123',
      marker_label: 'rename holon',
      snapshot_after: true,
      disable_undo: false,
    });

    expect(request).toEqual({
      request_id: 1,
      command: beginTransactionCommand,
      options: {
        marker_id: 'gesture-123',
        marker_label: 'rename holon',
        snapshot_after: true,
        disable_undo: false,
      },
    });
  });

  it('supports partial request option overrides', () => {
    const request = buildRequest(beginTransactionCommand, {
      marker_label: 'partial override',
    });

    expect(request.options).toEqual({
      marker_id: null,
      marker_label: 'partial override',
      snapshot_after: false,
      disable_undo: false,
    });
  });

  it('validates disable_undo as part of request options', () => {
    const request = buildRequest(beginTransactionCommand, {
      disable_undo: true,
    });

    expect(request.options.disable_undo).toBe(true);
  });

  it('increments request ids across successive buildRequest calls', () => {
    const first = buildRequest(beginTransactionCommand);
    const second = buildRequest(beginTransactionCommand);

    expect(first.request_id).toBe(1);
    expect(second.request_id).toBe(2);
  });
});
