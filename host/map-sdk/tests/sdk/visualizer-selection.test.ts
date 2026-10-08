import { describe, expect, it, vi } from 'vitest';
const { select, selectCollection, discover, choose } = vi.hoisted(() => ({ select: vi.fn(), selectCollection: vi.fn(), discover: vi.fn(), choose: vi.fn() }));
vi.mock('../../src/internal/commands/transaction', () => ({ selectVisualizer: select, selectCollectionVisualizer: selectCollection, discoverVisualizers: discover, chooseVisualizer: choose }));
import { createMapTransaction } from '../../src/sdk/transaction';
import { createHolonReference, unwrapHolonReference } from '../../src/sdk/references';
import { createPropertyDescriptorHandle, createHolonDescriptorHandle } from '../../src/sdk/descriptors';
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
      ? await transaction.selectVisualizer({ subject: reference, requestedKind: 'propertyMap', owner: { visualizer: parentReference }, theme: createHolonReference(41, subject), slot: createHolonReference(41, slot) })
      : kind === 'Property'
        ? await transaction.selectPropertyVisualizer(descriptor, parentReference, createHolonReference(41, slot), createHolonReference(41, subject))
        : await transaction.selectValueVisualizer(descriptor, parentReference, createHolonReference(41, slot), createHolonReference(41, subject));
    expect(select).toHaveBeenLastCalledWith(41, { subject, requested_kind: kind, owner: { Visualizer: parent }, theme: subject, slot });
    expect(unwrapHolonReference(result.selected)).toEqual(selected);
    expect(result.requestedKind).toBe(kind === 'PropertyMap' ? 'propertyMap' : kind.toLowerCase());
  });

  it('propagates Rust no-selection without choosing a local fallback', async () => {
    select.mockRejectedValue(new Error('No applicable Value Visualizer'));
    const descriptor = createPropertyDescriptorHandle(createHolonReference(41, subject));
    await expect(createMapTransaction(41).selectValueVisualizer(descriptor, createHolonReference(41, parent), createHolonReference(41, slot), createHolonReference(41, subject))).rejects.toThrow('No applicable Value Visualizer');
  });
});

it('selects projected values by their declared type through the ordinary Collection command', async () => {
  selectCollection.mockResolvedValue({ selected, requested_kind: 'Collection', alternatives_available: false });
  const tx = createMapTransaction(41);
  const result = await tx.selectProjectedCollectionVisualizer(createHolonReference(41, subject), createHolonReference(41, parent), createHolonReference(41, slot));
  expect(selectCollection).toHaveBeenCalledWith(41, { collection: { element_type: subject, members: { state: 'Fetched', members: [], keyed_index: {} } }, parent_visualizer: parent, slot });
  expect(unwrapHolonReference(result.selected)).toEqual(selected);
  expect(result.requestedKind).toBe('collection');
});
it('refuses to move transaction-local diagnostic subjects into a presentation transaction', () => {
  expect(() => createMapTransaction(42).bindSavedReference(createHolonReference(41, subject))).toThrow('persisted Smart');
});

it('binds canonical saved property descriptors through the public lookup before Value selection', async () => {
  const tx = createMapTransaction(41);
  const reference = createHolonReference(41, subject);
  const lookup = vi.spyOn(tx, 'getSavedHolonByBaseKey').mockResolvedValue(reference);
  const property = await tx.getSavedPropertyDescriptorByBaseKey('Key.PropertyType');
  select.mockResolvedValue({ selected, requested_kind: 'Value', alternatives_available: false });
  await tx.selectValueVisualizer(property!, createHolonReference(41, parent), createHolonReference(41, slot), createHolonReference(41, subject));
  expect(lookup).toHaveBeenCalledWith('Key.PropertyType');
  expect(select).toHaveBeenLastCalledWith(41, { subject, owner: { Visualizer: parent }, theme: subject, slot, requested_kind: 'Value' });
  lookup.mockResolvedValue(null);
  expect(await tx.getSavedPropertyDescriptorByBaseKey('Missing.PropertyType')).toBeNull();
});

it('does not borrow a transaction-local PropertyDescriptor from another context', async () => {
  const property = createPropertyDescriptorHandle(createHolonReference(41, subject));
  expect(() => createMapTransaction(42).selectValueVisualizer(property, createHolonReference(42, parent), createHolonReference(42, slot), createHolonReference(42, subject))).toThrow('persisted Smart');
});

it('selects retained collection types in a fresh context without sending loader-bound membership', async () => {
  const typeWire: HolonReferenceWire = { Smart: { holon_id: { Local: [8] }, smart_property_values: null } };
  const elementType = createHolonDescriptorHandle(createHolonReference(41, typeWire));
  selectCollection.mockResolvedValue({ selected, requested_kind: 'Collection', alternatives_available: false });
  await createMapTransaction(42).selectProjectedCollectionVisualizer(elementType, createHolonReference(42, parent), createHolonReference(42, slot));
  expect(selectCollection).toHaveBeenLastCalledWith(42, { collection: { element_type: typeWire,
    members: { state: 'Fetched', members: [], keyed_index: {} } }, parent_visualizer: parent, slot });
});


it('binds discovery provenance and keeps a stale current choice separate', async () => {
  const tx = createMapTransaction(41);
  const request = { subject: createHolonReference(41, subject), requestedKind: 'node' as const,
    owner: { dancer: createHolonReference(41, parent) }, slot: createHolonReference(41, slot), theme: createHolonReference(41, subject) };
  discover.mockResolvedValue({ candidates: [{ visualizer: selected, declared_on: [subject, parent], assessment: 'viable' }],
    current_selection: { visualizer: parent, declared_on: [], assessment: 'no_longer_applicable' }, ancestry: [subject, parent] });
  const result = await tx.discoverVisualizers(request, request.owner.dancer);
  expect(discover).toHaveBeenCalledWith(41, { subject, requested_kind: 'Node', owner: { Dancer: parent }, slot, theme: subject }, parent);
  expect(result.candidates[0].declaredOn.map(unwrapHolonReference)).toEqual([subject, parent]);
  expect(result.currentSelection?.assessment).toBe('no_longer_applicable');
  expect(result.ancestry.map(unwrapHolonReference)).toEqual([subject, parent]);
  choose.mockResolvedValue({ selected, requested_kind: 'Node', alternatives_available: false });
  const chosen = await tx.chooseVisualizer(request, result.candidates[0].visualizer);
  expect(choose).toHaveBeenCalledWith(41, expect.objectContaining({ owner: { Dancer: parent } }), selected);
  expect(unwrapHolonReference(chosen.selected)).toEqual(selected);
  choose.mockRejectedValue(new Error('Explicit Visualizer choice is not viable'));
  await expect(tx.chooseVisualizer(request, chosen.selected)).rejects.toThrow('not viable');
});
