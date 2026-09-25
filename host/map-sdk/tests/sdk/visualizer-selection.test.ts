import { describe, expect, it, vi } from 'vitest';
const { select } = vi.hoisted(() => ({ select: vi.fn() }));
vi.mock('../../src/internal/commands/transaction', () => ({ selectVisualizer: select }));
import { createMapTransaction } from '../../src/sdk/transaction';
import { createHolonReference, unwrapHolonReference } from '../../src/sdk/references';
import { createPropertyDescriptorHandle } from '../../src/sdk/descriptors';
import type { HolonReferenceWire } from '../../src/internal/wire-types';

const wire = (id: string): HolonReferenceWire => ({ Transient: { tx_id: 41, id } });
const subject = wire('00000000-0000-0000-0000-000000000001');
const parent = wire('00000000-0000-0000-0000-000000000002');
const slot = wire('00000000-0000-0000-0000-000000000004');
const selected = wire('00000000-0000-0000-0000-000000000003');

describe('descriptor selection request binding', () => {
  it.each(['PropertyMap', 'Property', 'Value'] as const)('projects %s subject and parent and binds the selected response', async kind => {
    select.mockResolvedValue({ selected, requested_kind: kind, alternatives_available: false });
    const transaction = createMapTransaction(41);
    const reference = createHolonReference(41, subject);
    const parentReference = createHolonReference(41, parent);
    const descriptor = createPropertyDescriptorHandle(reference);
    const result = kind === 'PropertyMap'
      ? await transaction.selectVisualizer({ subject: reference, requestedKind: 'propertyMap', parentVisualizer: parentReference, slot: createHolonReference(41, slot) })
      : kind === 'Property'
        ? await transaction.selectPropertyVisualizer(descriptor, parentReference, createHolonReference(41, slot))
        : await transaction.selectValueVisualizer(descriptor, parentReference, createHolonReference(41, slot));
    expect(select).toHaveBeenLastCalledWith(41, { subject, requested_kind: kind, parent_visualizer: parent, slot });
    expect(unwrapHolonReference(result.selected)).toEqual(selected);
    expect(result.requestedKind).toBe(kind === 'PropertyMap' ? 'propertyMap' : kind.toLowerCase());
  });

  it('propagates Rust no-selection without choosing a local fallback', async () => {
    select.mockRejectedValue(new Error('No applicable Value Visualizer'));
    const descriptor = createPropertyDescriptorHandle(createHolonReference(41, subject));
    await expect(createMapTransaction(41).selectValueVisualizer(descriptor, createHolonReference(41, parent), createHolonReference(41, slot))).rejects.toThrow('No applicable Value Visualizer');
  });
});
