import { describe, expect, it, vi } from 'vitest';
import { createPropertyDescriptorHandle, createRelationshipDescriptorHandle } from '../../src/sdk/descriptors';
import type { HolonReference } from '../../src/sdk/references';

describe('descriptor-backed value context', () => {
  it('projects the declared ValueType and property name through bound references', async () => {
    const valueType = { propertyValue: vi.fn().mockResolvedValue({ StringValue: 'PublicationStatus' }) };
    const property = {
      propertyValue: vi.fn().mockResolvedValue({ StringValue: 'Status' }),
      relatedHolons: vi.fn().mockResolvedValue({ length: 1, members: [valueType] }),
    };
    const descriptor = createPropertyDescriptorHandle(property as unknown as HolonReference);
    expect(await descriptor.propertyName()).toBe('Status');
    expect(await (await descriptor.valueType()).typeName()).toBe('PublicationStatus');
    expect(property.relatedHolons).toHaveBeenCalledWith('ValueType');
    expect(valueType.propertyValue).toHaveBeenCalledWith('TypeName');
  });

  it.each([0, 2])('rejects %i ValueType relationships', async count => {
    const property = { relatedHolons: vi.fn().mockResolvedValue({ length: count, members: Array(count).fill({}) }) };
    const descriptor = createPropertyDescriptorHandle(property as unknown as HolonReference);
    await expect(descriptor.valueType()).rejects.toThrow(`exactly one ValueType; found ${count}`);
  });
});

it('uses relationship DisplayName rather than the binding name', async () => {
  const propertyValue = vi.fn().mockResolvedValue({ StringValue: 'Owned by' });
  const descriptor = createRelationshipDescriptorHandle({ propertyValue } as unknown as HolonReference);
  expect(await descriptor.displayName()).toBe('Owned by');
  expect(propertyValue).toHaveBeenCalledWith('DisplayName');
});

it('reads optional relationship descriptions through the bound descriptor', async () => {
  const propertyValue = vi.fn().mockResolvedValueOnce({ StringValue: 'The owning HolonSpace.' }).mockResolvedValueOnce(null);
  const descriptor = createRelationshipDescriptorHandle({ propertyValue } as unknown as HolonReference);
  expect(await descriptor.description()).toBe('The owning HolonSpace.');
  expect(propertyValue).toHaveBeenCalledWith('Description');
  expect(await descriptor.description()).toBeNull();
});

it('reads resolved collections and contribution provenance from the retained graph', async () => {
  const { createEffectiveDescriptorHandle } = await import('../../src/sdk/descriptors');
  const member = { propertyValue: vi.fn().mockResolvedValue({ StringValue: 'Title' }) };
  const origin = {};
  const collection = {};
  const contribution = {
    propertyValue: vi.fn().mockResolvedValue({ IntegerValue: 0 }),
    relatedHolons: vi.fn(async (name: string) => ({ members: [{ ContributionCollection: collection, ContributionMember: member, ContributionDeclaredOn: origin }[name]] })),
  };
  const relationships: Record<string, unknown[]> = {
    EffectiveInstanceProperties: [member], EffectiveContributions: [contribution],
    EffectiveInstanceKeyRule: [member], EffectiveAffordsDance: [],
  };
  const reference = { relatedHolons: vi.fn(async (name: string) => ({ members: relationships[name] ?? [] })) };
  const descriptor = createEffectiveDescriptorHandle(reference as unknown as HolonReference);
  const properties = await descriptor.instanceProperties();
  expect(Object.isFrozen(properties)).toBe(true);
  expect(await properties[0].propertyName()).toBe('Title');
  const records = await descriptor.contributions();
  expect(await records[0].collection()).toBe(collection);
  expect(await records[0].member()).toBe(member);
  expect(await records[0].declaredOn()).toBe(origin);
  expect(await records[0].position()).toBe(0);
  expect(await descriptor.effectiveKeyRule()).toBe(member);
  expect(await descriptor.affordedDances()).toEqual([]);
  // Reads remain reference-backed rather than retaining a second collection authority.
  relationships['EffectiveInstanceProperties'] = [];
  expect(await descriptor.instanceProperties()).toEqual([]);
});

it('preserves requiredness and lifecycle errors on resolved results', async () => {
  const { createEffectiveDescriptorHandle } = await import('../../src/sdk/descriptors');
  const relatedHolons = vi.fn().mockResolvedValue({ members: [] });
  const descriptor = createEffectiveDescriptorHandle({ relatedHolons } as unknown as HolonReference);
  await expect(descriptor.effectiveKeyRule()).rejects.toThrow('exactly one');
  const disposed = new Error('transaction disposed');
  relatedHolons.mockRejectedValue(disposed);
  await expect(descriptor.instanceProperties()).rejects.toBe(disposed);
});

it('indexes each effective vocabulary once and preserves ambiguity without hiding other names', async () => {
  const { createEffectiveDescriptorHandle } = await import('../../src/sdk/descriptors');
  const member = (name: string) => ({ propertyValue: vi.fn().mockResolvedValue({ StringValue: name }) });
  const repeated = member('Name');
  const conflicting = member('Name');
  const unique = member('Other');
  const reference = { relatedHolons: vi.fn().mockResolvedValue({ members: [repeated, repeated, conflicting, unique] }) };
  const descriptor = createEffectiveDescriptorHandle(reference as unknown as HolonReference);
  await expect(descriptor.propertyNamed('Name')).rejects.toThrow('3 declaration occurrences');
  const other = await descriptor.propertyNamed('Other');
  expect(other).not.toBeNull();
  expect(await other!.propertyName()).toBe('Other');
  expect(await descriptor.propertyNamed('Missing')).toBeNull();
  expect(reference.relatedHolons).toHaveBeenCalledTimes(1);
  expect(conflicting.propertyValue).toHaveBeenCalledTimes(1);
});

it.each(['relationshipNamed', 'danceNamed', 'commandNamed'] as const)('rejects conflicting %s lookups', async method => {
  const { createEffectiveDescriptorHandle } = await import('../../src/sdk/descriptors');
  const member = { propertyValue: vi.fn().mockResolvedValue({ StringValue: 'Repeated' }) };
  const descriptor = createEffectiveDescriptorHandle({ relatedHolons: vi.fn().mockResolvedValue({ members: [member, member] }) } as unknown as HolonReference);
  await expect(descriptor[method]('Repeated')).rejects.toThrow('Ambiguous');
});
