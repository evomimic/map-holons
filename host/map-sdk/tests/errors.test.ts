import { describe, expect, it } from 'vitest';

import {
  DomainError,
  MalformedResponseError,
  MapError,
  TransportError,
  parseDomainError,
} from '../src/internal/errors';

// ===========================================
// Internal Error Hierarchy Tests
// ===========================================

describe('errors', () => {
  it('builds transport errors with code and cause', () => {
    const cause = new Error('invoke failed');
    const error = new TransportError('Transport failure', cause);

    expect(error).toBeInstanceOf(Error);
    expect(error).toBeInstanceOf(MapError);
    expect(error).toBeInstanceOf(TransportError);
    expect(error.name).toBe('TransportError');
    expect(error.code).toBe('TRANSPORT_ERROR');
    expect(error.message).toBe('Transport failure');
    expect(error.cause).toBe(cause);
  });

  it('builds malformed response errors with details', () => {
    const details = { request_id: 3, response_id: 4 };
    const error = new MalformedResponseError('Request/response mismatch', details);

    expect(error).toBeInstanceOf(MapError);
    expect(error).toBeInstanceOf(MalformedResponseError);
    expect(error.code).toBe('MALFORMED_RESPONSE');
    expect(error.details).toEqual(details);
  });

  it('builds domain errors with variant and payload', () => {
    const error = new DomainError('HolonNotFound', 'missing-holon');

    expect(error).toBeInstanceOf(MapError);
    expect(error).toBeInstanceOf(DomainError);
    expect(error.code).toBe('DOMAIN_ERROR');
    expect(error.variant).toBe('HolonNotFound');
    expect(error.payload).toBe('missing-holon');
    expect(error.message).toContain('HolonNotFound');
  });

  it('allows a custom domain error message', () => {
    const error = new DomainError('TransactionNotOpen', { tx_id: 41 }, 'tx closed');

    expect(error.message).toBe('tx closed');
    expect(error.variant).toBe('TransactionNotOpen');
    expect(error.payload).toEqual({ tx_id: 41 });
  });

  it('parses a simple wire domain error', () => {
    const error = parseDomainError({ HolonNotFound: 'missing-holon' });

    expect(error).toBeInstanceOf(DomainError);
    expect(error.code).toBe('DOMAIN_ERROR');
    expect(error.variant).toBe('HolonNotFound');
    expect(error.payload).toBe('missing-holon');
  });

  it('parses a structured wire domain error payload', () => {
    const error = parseDomainError({
      CrossTransactionReference: {
        reference_kind: 'Staged',
        reference_id: '22222222-2222-2222-2222-222222222222',
        reference_tx: 41,
        context_tx: 99,
      },
    });

    expect(error.variant).toBe('CrossTransactionReference');
    expect(error.payload).toEqual({
      reference_kind: 'Staged',
      reference_id: '22222222-2222-2222-2222-222222222222',
      reference_tx: 41,
      context_tx: 99,
    });
  });

  it('parses nested validation errors without flattening the payload', () => {
    const error = parseDomainError({
      ValidationError: {
        PropertyError: 'title is required',
      },
    });

    expect(error.variant).toBe('ValidationError');
    expect(error.payload).toEqual({
      PropertyError: 'title is required',
    });
  });
});
