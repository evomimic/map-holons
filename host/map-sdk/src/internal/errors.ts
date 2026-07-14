import { type HolonErrorWire } from './wire-types';

// ===========================================
// Internal MAP Error Hierarchy
// ===========================================

export type MapErrorCode =
  | 'TRANSPORT_ERROR'
  | 'MALFORMED_RESPONSE'
  | 'DOMAIN_ERROR';

/**
 * Base class for internal SDK command-layer failures.
 *
 * This hierarchy exists so tests can assert on failure kind without relying on
 * free-form error message text.
 */
export abstract class MapError extends Error {
  abstract readonly code: MapErrorCode;

  protected constructor(message: string) {
    super(message);
    this.name = new.target.name;
  }
}

/**
 * Raised when the Tauri invoke layer rejects or fails to deserialize.
 */
export class TransportError extends MapError {
  readonly code = 'TRANSPORT_ERROR' as const;
  override readonly cause: unknown;

  constructor(message: string, cause?: unknown) {
    super(message);
    this.cause = cause;
  }
}

/**
 * Raised when the Rust response is structurally invalid or does not correlate
 * with the originating request.
 */
export class MalformedResponseError extends MapError {
  readonly code = 'MALFORMED_RESPONSE' as const;
  readonly details: unknown;

  constructor(message: string, details?: unknown) {
    super(message);
    this.details = details;
  }
}

/**
 * Raised when Rust returns a domain-level `HolonError`.
 */
export class DomainError extends MapError {
  readonly code = 'DOMAIN_ERROR' as const;
  readonly variant: string;
  readonly payload: unknown;

  constructor(variant: string, payload: unknown, message?: string) {
    super(message ?? `MAP domain error: ${variant}`);
    this.variant = variant;
    this.payload = payload;
  }
}

// ===========================================
// Error Conversion
// ===========================================

/**
 * Convert a decoded wire-level HolonError into the internal DomainError type.
 */
export function parseDomainError(wire: HolonErrorWire): DomainError {
  const [variant, payload] = Object.entries(wire)[0] ?? ['UnknownDomainError', undefined];
  return new DomainError(variant, payload, formatDomainErrorMessage(variant, payload));
}

function formatDomainErrorMessage(variant: string, payload: unknown): string {
  const detail = formatDomainErrorPayload(payload);
  return detail ? `${variant}: ${detail}` : `MAP domain error: ${variant}`;
}

function formatDomainErrorPayload(payload: unknown): string {
  if (typeof payload === 'string') {
    return payload;
  }

  if (Array.isArray(payload)) {
    return payload.map((item) => formatDomainErrorPayload(item)).join(': ');
  }

  if (payload && typeof payload === 'object') {
    const entries = Object.entries(payload as Record<string, unknown>);
    if (entries.length === 1) {
      const [key, value] = entries[0];
      const nested = formatDomainErrorPayload(value);
      return nested ? `${key}: ${nested}` : key;
    }

    return entries
      .map(([key, value]) => `${key}=${formatDomainErrorPayload(value)}`)
      .join(', ');
  }

  if (payload === null || payload === undefined) {
    return '';
  }

  return String(payload);
}
