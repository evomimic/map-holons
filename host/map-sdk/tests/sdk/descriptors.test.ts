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
  const { createEffectiveHolonDescriptorHandle } = await import('../../src/sdk/descriptors');
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
  const descriptor = createEffectiveHolonDescriptorHandle(reference as unknown as HolonReference);
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
  const { createEffectiveHolonDescriptorHandle } = await import('../../src/sdk/descriptors');
  const relatedHolons = vi.fn().mockResolvedValue({ members: [] });
  const descriptor = createEffectiveHolonDescriptorHandle({ relatedHolons } as unknown as HolonReference);
  await expect(descriptor.effectiveKeyRule()).rejects.toThrow('exactly one');
  const disposed = new Error('transaction disposed');
  relatedHolons.mockRejectedValue(disposed);
  await expect(descriptor.instanceProperties()).rejects.toBe(disposed);
});

it('delegates named lookup to Rust without enumerating schema members in TypeScript', async () => {
  const { createEffectiveHolonDescriptorHandle } = await import('../../src/sdk/descriptors');
  const commands = await import('../../src/internal/commands/holon');
  const { createHolonReference } = await import('../../src/sdk/references');
  const wire = { Transient: { tx_id: 41, id: '00000000-0000-0000-0000-000000000001' } } as const;
  const lookup = vi.spyOn(commands, 'readEffectiveMember').mockResolvedValue(null);
  const descriptor = createEffectiveHolonDescriptorHandle(createHolonReference(41, wire));
  await expect(descriptor.propertyNamed('Missing')).resolves.toBeNull();
  expect(lookup).toHaveBeenCalledWith(41, wire, 'EffectiveInstanceProperties', 'Missing');
  lookup.mockRestore();
});

it('routes retained descriptor reads through their actual owner after acquisition', async () => {
  const commands = await import('../../src/internal/commands/holon');
  const { createHolonReference } = await import('../../src/sdk/references');
  const sourceWire = { Transient: { tx_id: 41, id: '00000000-0000-0000-0000-000000000001' } } as const;
  const preparedWire = { Transient: { tx_id: 77, id: '00000000-0000-0000-0000-000000000002' } } as const;
  const acquire = vi.spyOn(commands, 'readEffectiveDescriptor').mockResolvedValue(preparedWire);
  const read = vi.spyOn(commands, 'readRelatedHolons').mockResolvedValue({ state: 'Fetched', members: [], keyed_index: {} });
  const descriptor = await createHolonReference(41, sourceWire).effectiveDescriptor();
  expect(acquire).toHaveBeenCalledWith(41, sourceWire);
  await expect(descriptor.instanceProperties()).resolves.toEqual([]);
  expect(read).toHaveBeenCalledWith(77, preparedWire, 'EffectiveInstanceProperties', undefined, false);
  acquire.mockRestore(); read.mockRestore();
});
