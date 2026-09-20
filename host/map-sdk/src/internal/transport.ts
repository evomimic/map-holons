import { invoke } from '@tauri-apps/api/core';

import {
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
 * - validate structural response shape
 * - correlate response.request_id with the originating request
 * - map transport and malformed-response failures into the internal
 *   SDK error hierarchy
 */
export async function invokeMapCommand(
  request: MapIpcRequest,
): Promise<MapIpcResponse> {
  let response: unknown;
  const profiling = performance.getEntriesByName('map.startup.active', 'mark').length > 0;
  const started = profiling ? performance.now() : 0;

  try {
    response = await invoke<MapIpcResponse>('dispatch_map_command', { request });
  } catch (cause) {
    throw new TransportError('Failed to invoke dispatch_map_command', cause);
  } finally {
    if (profiling) {
      performance.measure('map.ipc', {
        start: started,
        end: performance.now(),
        detail: commandProfileLabel(request.command),
      });
    }
  }

  if (!isRecord(response) || !isNumber(response['request_id'])) {
    throw new MalformedResponseError(
      'MAP IPC response is missing a valid request_id',
      response,
    );
  }

  if (response['request_id'] !== request.request_id) {
    throw new MalformedResponseError(
      'MAP IPC response request_id did not match the originating request',
      {
        request_id: request.request_id,
        response_request_id: response['request_id'],
      },
    );
  }

  return response as unknown as MapIpcResponse;
}

// ===========================================
// Response Unwrap
// ===========================================

/**
 * Interpret the Ok/Err result envelope inside a validated `MapIpcResponse`.
 *
 * - Ok payload → returned as `MapResultWire`
 * - Err payload → thrown as `DomainError`
 * - Anything else → thrown as `MalformedResponseError`
 */
export function unwrapMapResponse(response: MapIpcResponse): MapResultWire {
  const result = response.result;

  if (!isRecord(result)) {
    throw new MalformedResponseError(
      'MAP IPC response is missing a valid result envelope',
      response,
    );
  }

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


/** Records only operation names, never command payloads or holon values. */
function commandProfileLabel(command: MapIpcRequest['command']): string {
  const [scope, body] = Object.entries(command)[0];
  let action: unknown = isRecord(body) ? body['action'] : body;
  const parts = [scope];
  for (let depth = 0; depth < 2; depth++) {
    if (typeof action === 'string') { parts.push(action); break; }
    if (!isRecord(action)) break;
    const [name, child] = Object.entries(action)[0] ?? [];
    if (!name) break;
    parts.push(name);
    if (name !== 'Read' && name !== 'Write') break;
    action = child;
  }
  return parts.join('.');
}
