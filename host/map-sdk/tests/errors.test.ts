import { describe, expect, it } from 'vitest';
import { isMapIpcResponse, type HolonErrorWire } from '../src/internal/wire-types';

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

// Explicit fixtures match integrity_core_types' query_errors_preserve_sdk_json_contract_in_both_directions.
describe('Query domain error wire contract', () => {
  it('accepts Rust serde errors through the transport envelope and retains domain payloads', () => {
    const errors: HolonErrorWire[] = [
      {ValueConstraintViolation: {code: 'StringLengthOutOfRange', value_type: 'value-type', constraint: 'constraint', constraint_type: 'constraint-type', descriptor: 'descriptor', detail: 'measured 6 graphemes; required [0, 5]'}},
      {UnsupportedValueConstraint: {constraint_type: 'constraint-type', value_type: 'value-type', descriptor: 'descriptor'}},
      {IncompatibleLiteralType: {declared: 'declared', required: 'required', operator: 'Contains'}},
      {ValueKindMismatch: {expected: 'String', found: 'Integer', descriptor: 'descriptor'}},
      {MultipleDescribedBy: {holon: 'holon', count: 2}},
      {OperandCountMismatch: {operator: 'Contains', expected: 1, found: 2}},
      {MissingDescribedBy: {holon: 'holon'}},
    ];
    for (const kind of ['ContradictoryConstraints', 'MissingConstraintParameter', 'IncompatibleConstraintFamily', 'UnsupportedExecutableConstraint', 'UnclassifiedConstraint', 'InvalidConstraintConfiguration'] as const) {
      errors.push({ DescriptorSchemaInvalid: { kind, descriptor: 'descriptor', detail: 'invalid configuration' } });
    }
    for (const error of errors) {
      const wire = JSON.parse(JSON.stringify(error));
      expect(isMapIpcResponse({ request_id: 1, result: { Err: wire } })).toBe(true);
      const domain = parseDomainError(wire);
      const [variant, payload] = Object.entries(error)[0];
      expect(domain.variant).toBe(variant);
      expect(domain.payload).toEqual(payload);
    }
  });
  it('rejects malformed query errors at the transport boundary', () => {
    for (const error of [
      { ValueConstraintViolation: { code: 'code' } },
      { DescriptorSchemaInvalid: { kind: 'Unknown', descriptor: 'descriptor', detail: 'detail' } },
      { UnsupportedValueConstraint: { constraint_type: 'type', value_type: 'value' } },
      { IncompatibleLiteralType: { declared: 1, required: 'required', operator: 'Contains' } },
      { ValueKindMismatch: { expected: 'String', found: 1, descriptor: 'descriptor' } },
      { MultipleDescribedBy: { holon: 'holon', count: 'two' } },
      { OperandCountMismatch: { operator: 'Contains', expected: 1 } },
    ]) {
      expect(isMapIpcResponse({ request_id: 1, result: { Err: error } })).toBe(false);
    }
  });
});
