import { type MapCommandWire, isMapCommandWire } from './commands';
import {
  type HolonErrorWire,
  isHolonErrorWire,
  isNumber,
  isRecord,
} from './references';
import { type MapResultWire, isMapResultWire } from './results';

// ===========================================
// IPC Envelope Types
// ===========================================

// Transparent newtype over Rust RequestId(MapInteger(i64)).
export type RequestId = number;

/**
 * Per-request dispatch options echoed by the Rust IPC envelope.
 */
export interface RequestOptions {
  gesture_id: string | null;
  gesture_label: string | null;
  snapshot_after: boolean;
}

/**
 * Canonical request shape sent to `dispatch_map_command`.
 */
export interface MapIpcRequest {
  request_id: RequestId;
  command: MapCommandWire;
  options: RequestOptions;
}

/**
 * Serde shape for `Result<T, E>` at the IPC boundary.
 */
export type WireResult<T, E> = { Ok: T } | { Err: E };

/**
 * Canonical response shape returned from `dispatch_map_command`.
 */
export interface MapIpcResponse {
  request_id: RequestId;
  result: WireResult<MapResultWire, HolonErrorWire>;
}

// ===========================================
// Envelope Guards
// ===========================================

export function isRequestOptions(value: unknown): value is RequestOptions {
  return (
    isRecord(value) &&
    (value['gesture_id'] === null || typeof value['gesture_id'] === 'string') &&
    (value['gesture_label'] === null || typeof value['gesture_label'] === 'string') &&
    typeof value['snapshot_after'] === 'boolean'
  );
}

export function isWireResult<T, E>(
  value: unknown,
  okGuard: (candidate: unknown) => candidate is T,
  errGuard: (candidate: unknown) => candidate is E,
): value is WireResult<T, E> {
  return (
    (isRecord(value) && Object.keys(value).length === 1 && 'Ok' in value && okGuard(value['Ok'])) ||
    (isRecord(value) && Object.keys(value).length === 1 && 'Err' in value && errGuard(value['Err']))
  );
}

export function isMapIpcRequest(value: unknown): value is MapIpcRequest {
  return (
    isRecord(value) &&
    isNumber(value['request_id']) &&
    isMapCommandWire(value['command']) &&
    isRequestOptions(value['options'])
  );
}

export function isMapIpcResponse(value: unknown): value is MapIpcResponse {
  return (
    isRecord(value) &&
    isNumber(value['request_id']) &&
    isWireResult(value['result'], isMapResultWire, isHolonErrorWire)
  );
}
