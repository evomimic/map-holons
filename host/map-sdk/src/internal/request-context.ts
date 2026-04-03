import { type MapCommandWire, type MapIpcRequest, type RequestOptions } from './wire-types';

// ===========================================
// Request Context Defaults
// ===========================================

// Monotonic client-side request counter used for IPC correlation.
let requestIdCounter = 1;

/**
 * Optional overrides accepted by buildRequest().
 */
export type RequestOptionsOverrides = Partial<RequestOptions>;

/**
 * Return the next request id for a MAP IPC request.
 */
export function nextRequestId(): number {
  const current = requestIdCounter;
  requestIdCounter += 1;
  return current;
}

/**
 * Default request metadata attached to every MAP IPC command.
 */
export function defaultRequestOptions(): RequestOptions {
  return {
    gesture_id: null,
    gesture_label: null,
    snapshot_after: false,
  };
}

/**
 * Build a complete IPC envelope from a structural MAP command plus optional
 * RequestOptions overrides.
 */
export function buildRequest(
  command: MapCommandWire,
  options?: RequestOptionsOverrides,
): MapIpcRequest {
  return {
    request_id: nextRequestId(),
    command,
    options: {
      ...defaultRequestOptions(),
      ...options,
    },
  };
}

/**
 * Reset the request id counter for deterministic unit tests.
 */
export function resetRequestIdCounter(): void {
  requestIdCounter = 1;
}
