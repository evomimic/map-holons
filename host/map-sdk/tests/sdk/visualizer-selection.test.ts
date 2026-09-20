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
const selected = wire('00000000-0000-0000-0000-000000000003');

describe('descriptor selection request binding', () => {
  it.each(['Properties', 'Property', 'Value'] as const)('projects %s subject and parent and binds the selected response', async kind => {
    select.mockResolvedValue({ selected, requested_kind: kind, alternatives_available: false });
    const transaction = createMapTransaction(41);
    const reference = createHolonReference(41, subject);
    const parentReference = createHolonReference(41, parent);
    const descriptor = createPropertyDescriptorHandle(reference);
    const result = kind === 'Properties'
      ? await transaction.selectVisualizer({ subject: reference, requestedKind: 'properties', parentVisualizer: parentReference })
      : kind === 'Property'
        ? await transaction.selectPropertyVisualizer(descriptor, parentReference)
        : await transaction.selectValueVisualizer(descriptor, parentReference);
    expect(select).toHaveBeenLastCalledWith(41, { subject, requested_kind: kind, parent_visualizer: parent });
    expect(unwrapHolonReference(result.selected)).toEqual(selected);
    expect(result.requestedKind).toBe(kind.toLowerCase());
  });

  it('propagates Rust no-selection without choosing a local fallback', async () => {
    select.mockRejectedValue(new Error('No applicable Value Visualizer'));
    const descriptor = createPropertyDescriptorHandle(createHolonReference(41, subject));
    await expect(createMapTransaction(41).selectValueVisualizer(descriptor, createHolonReference(41, parent))).rejects.toThrow('No applicable Value Visualizer');
  });
});
