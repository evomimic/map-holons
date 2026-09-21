import { describe, expect, it, vi } from 'vitest';
import { classifyNodeAffordances } from './classify-node-affordances';
import type { HolonViewAccess } from '../contracts/holon-view';

function fixture() {
  const property = (name: string, array: boolean) => ({ propertyName: async () => name, isArray: async () => array });
  const relationship = (label: string, maximum: number | null, direction = 'declared') => ({
    direction, descriptor: { displayName: async () => label, effectiveCardinality: async () => ({ minimum: 0, maximum }) },
  });
  return {
    availableProperties: vi.fn().mockResolvedValue([property('Name', false), property('Tags', true)]),
    availableRelationships: vi.fn().mockResolvedValue([relationship('Hidden', 0), relationship('Parent', 1), relationship('Children', null), relationship('Sources', 3, 'inverse')]),
    availableDances: vi.fn().mockResolvedValue([{ propertyValue: async () => ({ StringValue: 'Friendly dance' }), versionedKey: async () => 'UnknownDance@1' }]),
    propertyValue: vi.fn(() => { throw Error('Must not fetch values'); }),
    expandRelationship: vi.fn(() => { throw Error('Must not expand targets'); }),
  };
}
describe('descriptor-driven Node classification', () => {
  it('populates both navigation regions, omits maximum zero, and discovers actions without reading contents or response metadata', async () => {
    const view = fixture();
    const result = await classifyNodeAffordances(view as unknown as HolonViewAccess);
    expect(result.scalarProperties).toHaveLength(1);
    expect(result.singularRelationships.map(x => x.label)).toEqual(['Parent']);
    expect(result.collections.map(x => x.label)).toEqual(['Tags', 'Children', 'Sources']);
    expect(result.collections[2]).toMatchObject({ relationship: { direction: 'inverse' } });
    expect(result.actions).toMatchObject([{ kind: 'action', label: 'Friendly dance' }]);
    expect(view.propertyValue).not.toHaveBeenCalled();
    expect(view.expandRelationship).not.toHaveBeenCalled();
  });
  it('propagates metadata errors instead of guessing cardinality', async () => {
    const view = fixture();
    view.availableRelationships.mockRejectedValue(new Error('Invalid cardinality'));
    await expect(classifyNodeAffordances(view as unknown as HolonViewAccess)).rejects.toThrow('Invalid cardinality');
  });
});
