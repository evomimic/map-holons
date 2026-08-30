import { describe, expect, it, vi } from 'vitest';

import type { HolonReference } from '../deps';
import { DahnHolonView } from './dahn-holon-view';

describe('DahnHolonView', () => {
  it('provides a read-only view that delegates live reads to the public SDK handle', async () => {
    const descriptor = {};
    const properties = [{}];
    const relationships = [{ descriptor: {}, direction: 'declared' as const }];
    const related = { members: [] };
    const reference = {
      holonId: vi.fn().mockResolvedValue({ Local: [1] }),
      key: vi.fn().mockResolvedValue('alpha'),
      versionedKey: vi.fn().mockResolvedValue('alpha@1'),
      propertyValue: vi.fn().mockResolvedValue({ StringValue: 'value' }),
      holonDescriptor: vi.fn().mockResolvedValue(descriptor),
      availableProperties: vi.fn().mockResolvedValue(properties),
      availableRelationships: vi.fn().mockResolvedValue(relationships),
      relatedHolons: vi.fn().mockResolvedValue(related),
    } as unknown as HolonReference;
    const view = new DahnHolonView(reference);

    await expect(view.key()).resolves.toBe('alpha');
    await expect(view.propertyValue('Title')).resolves.toEqual({ StringValue: 'value' });
    await expect(view.descriptor()).resolves.toBe(descriptor);
    await expect(view.availableProperties()).resolves.toBe(properties);
    await expect(view.availableRelationships()).resolves.toBe(relationships);
    await expect(view.expandRelationship('Contains')).resolves.toBe(related);

    expect(reference.key).toHaveBeenCalledOnce();
    expect(reference.propertyValue).toHaveBeenCalledWith('Title');
    expect(reference.holonDescriptor).toHaveBeenCalledOnce();
    expect(reference.availableProperties).toHaveBeenCalledOnce();
    expect(reference.availableRelationships).toHaveBeenCalledOnce();
    expect(reference.relatedHolons).toHaveBeenCalledWith('Contains');
    expect('withPropertyValue' in view).toBe(false);
  });

  it('propagates SDK errors without wrapping them', async () => {
    const error = new Error('MAP unavailable');
    const reference = {
      key: vi.fn().mockRejectedValue(error),
    } as unknown as HolonReference;

    await expect(new DahnHolonView(reference).key()).rejects.toBe(error);
  });
});
