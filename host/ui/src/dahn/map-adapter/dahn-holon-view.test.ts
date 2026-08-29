import { describe, expect, it, vi } from 'vitest';

import type { HolonReference } from '../deps';
import { DahnHolonView } from './dahn-holon-view';

describe('DahnHolonView', () => {
  it('provides a read-only view that delegates live reads to the public SDK handle', async () => {
    const reference = {
      holonId: vi.fn().mockResolvedValue({ Local: [1] }),
      key: vi.fn().mockResolvedValue('alpha'),
      versionedKey: vi.fn().mockResolvedValue('alpha@1'),
      propertyValue: vi.fn().mockResolvedValue({ StringValue: 'value' }),
      holonDescriptor: vi.fn().mockResolvedValue({}),
      availableProperties: vi.fn().mockResolvedValue([]),
      availableRelationships: vi.fn().mockResolvedValue([]),
      relatedHolons: vi.fn().mockResolvedValue({ members: [] }),
    } as unknown as HolonReference;
    const view = new DahnHolonView(reference);

    await expect(view.key()).resolves.toBe('alpha');
    await expect(view.propertyValue('Title')).resolves.toEqual({ StringValue: 'value' });
    await expect(view.expandRelationship('Contains')).resolves.toEqual({ members: [] });

    expect(reference.key).toHaveBeenCalledOnce();
    expect(reference.propertyValue).toHaveBeenCalledWith('Title');
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
