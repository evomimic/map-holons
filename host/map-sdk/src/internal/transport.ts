import { invoke } from '@tauri-apps/api/core';

import {
  DomainError,
  MalformedResponseError,
  TransportError,
  parseDomainError,
} from './errors';
import {
  type MapIpcRequest,
  type MapIpcResponse,
  type MapResultWire,
  hasSingleKey,
  isHolonErrorWire,
  isMapResultWire,
  isNumber,
  isRecord,
} from './wire-types';

// ===========================================
// MAP IPC Transport
// ===========================================

/**
 * Single TypeScript-side IPC boundary for MAP command execution.
 *
 * Responsibilities:
 * - call the Tauri `dispatch_map_command` entrypoint
 * - correlate response.request_id with the originating request
 * - unwrap WireResult Ok/Err payloads
 * - map transport, malformed-response, and domain failures into the internal
 *   SDK error hierarchy
 */
export async function invokeMapCommand(
  request: MapIpcRequest,
): Promise<MapResultWire> {
  let response: unknown;

  try {
    response = await invoke<MapIpcResponse>('dispatch_map_command', { request });
  } catch (cause) {
    throw new TransportError('Failed to invoke dispatch_map_command', cause);
  }

  if (!isRecord(response) || !isNumber(response.request_id)) {
    throw new MalformedResponseError(
      'MAP IPC response is missing a valid request_id',
      response,
    );
  }

  if (response.request_id !== request.request_id) {
    throw new MalformedResponseError(
      'MAP IPC response request_id did not match the originating request',
      {
        request_id: request.request_id,
        response_request_id: response.request_id,
      },
    );
  }

  if (!('result' in response) || !isRecord(response.result)) {
    throw new MalformedResponseError(
      'MAP IPC response is missing a valid result envelope',
      response,
    );
  }

  const result = response.result;

  if (hasSingleKey(result, 'Ok') && isMapResultWire(result.Ok)) {
    return result.Ok;
  }

  if (hasSingleKey(result, 'Err') && isHolonErrorWire(result.Err)) {
    throw parseDomainError(result.Err);
  }

  throw new MalformedResponseError(
    'MAP IPC response result envelope was malformed',
    response,
  );
}
