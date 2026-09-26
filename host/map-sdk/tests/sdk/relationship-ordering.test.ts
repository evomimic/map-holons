import { expect, it, vi } from 'vitest';
import { createRelationshipDescriptorHandle, createHolonDescriptorHandle } from '../../src/sdk/descriptors';
import { createHolonReference } from '../../src/sdk/references';
import { isMapCommandWire } from '../../src/internal/wire-types/commands';
import type { MapIpcRequest } from '../../src/internal/wire-types';

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
const target = { Transient: { tx_id: 41, id: 'fcb56a31-c1cb-4066-b4c3-d185809c2864' } };

it.each([true, false])('reads isOrdered=%s through the bound descriptor command', async ordered => {
  invoke.mockImplementation(async (_command, { request }: { request: MapIpcRequest }) => {
    expect(isMapCommandWire(request.command)).toBe(true);
    expect(request.command).toEqual({ Holon: {
      tx_id: 41, target, action: { Read: 'GetRelationshipIsOrdered' },
    } });
    return { request_id: request.request_id, result: { Ok: { Value: { BooleanValue: ordered } } } };
  });
  const descriptor = createRelationshipDescriptorHandle(createHolonReference(41, target));
  expect(await descriptor.isOrdered()).toBe(ordered);
});

it('rejects a non-boolean ordering response', async () => {
  invoke.mockImplementation(async (_command, { request }: { request: MapIpcRequest }) => ({
    request_id: request.request_id, result: { Ok: { Value: { StringValue: 'true' } } },
  }));
  const descriptor = createRelationshipDescriptorHandle(createHolonReference(41, target));
  await expect(descriptor.isOrdered()).rejects.toThrow('Expected BooleanValue result');
});

it.each([true, false])('reads hasInstanceKey=%s through the bound HolonType descriptor', async keyed => {
  invoke.mockImplementation(async (_command, { request }: { request: MapIpcRequest }) => {
    expect(isMapCommandWire(request.command)).toBe(true);
    expect(request.command).toEqual({ Holon: { tx_id: 41, target, action: { Read: 'GetHasInstanceKey' } } });
    return { request_id: request.request_id, result: { Ok: { Value: { BooleanValue: keyed } } } };
  });
  expect(await createHolonDescriptorHandle(createHolonReference(41, target)).hasInstanceKey()).toBe(keyed);
});

it('preserves a missing key rule as a domain error rather than a malformed response', async () => {
  invoke.mockImplementation(async (_command, { request }: { request: MapIpcRequest }) => ({
    request_id: request.request_id,
    result: { Err: { NoEffectiveKeyRule: { descriptor: 'DeclaredRelationshipType.RelationshipType' } } },
  }));
  const descriptor = createHolonDescriptorHandle(createHolonReference(41, target));
  await expect(descriptor.hasInstanceKey()).rejects.toMatchObject({
    code: 'DOMAIN_ERROR', variant: 'NoEffectiveKeyRule',
    payload: { descriptor: 'DeclaredRelationshipType.RelationshipType' },
  });
});

it.each([
  { NoEffectiveKeyRule: { descriptor: 123 } },
  { WrongDescriptorKind: { expected: 'KeyRuleType', found: 'Other' } },
])('still rejects malformed descriptor error payloads', async error => {
  invoke.mockImplementation(async (_command, { request }: { request: MapIpcRequest }) => ({ request_id: request.request_id, result: { Err: error } }));
  await expect(createHolonDescriptorHandle(createHolonReference(41, target)).hasInstanceKey()).rejects.toMatchObject({ code: 'MALFORMED_RESPONSE' });
});

it('preserves a wrong-kind descriptor diagnostic', async () => {
  const payload = { expected: 'KeyRuleType', found: 'Other', descriptor: 'Example' };
  invoke.mockImplementation(async (_command, { request }: { request: MapIpcRequest }) => ({ request_id: request.request_id, result: { Err: { WrongDescriptorKind: payload } } }));
  await expect(createHolonDescriptorHandle(createHolonReference(41, target)).hasInstanceKey()).rejects.toMatchObject({ code: 'DOMAIN_ERROR', variant: 'WrongDescriptorKind', payload });
});
