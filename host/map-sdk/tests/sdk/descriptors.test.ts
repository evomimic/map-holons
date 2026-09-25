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
