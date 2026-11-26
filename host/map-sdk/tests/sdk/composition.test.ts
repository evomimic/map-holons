import { DescribedHolonCollection } from '../../src/sdk/collection';
import { beforeEach, expect, it, vi } from 'vitest';
import { createHolonReference } from '../../src/sdk/references';
import { createPropertyDescriptorHandle, createRelationshipDescriptorHandle } from '../../src/sdk/descriptors';
import { createMapTransaction } from '../../src/sdk/transaction';
import type { HolonReferenceWire } from '../../src/internal/wire-types';
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
const ref = (id: number): HolonReferenceWire => ({ Smart: { holon_id: { Local: [id] }, smart_property_values: null } });
beforeEach(() => vi.resetAllMocks());
it('consumes consolidated child metadata and optional configuration through one boundary call', async () => {
  const references = Array.from({ length: 7 }, (_,i) => ref(i+1));
  const response = { references, data: { subject_properties: [], populations: [{ source: 0, relationship: "HasPersonalVisualizerPreference", members: [] }], theme: 0, work: { selections: 1, binding_reuses: 0, metadata_reuses: 2, personal_population_requests: 1, candidate_population_requests: 0 }, reusable_across_rounds: true, metadata: references.map((_,i) => ({ reference: i, key: `Definition${i}`, identity: `version-${i}`, versioned_key: `Definition${i}-v2`, properties: { DisplayName: { StringValue: `Name${i}` } } })),
    bindings: [{ parent: null, owner: 0, slot: 1, subject: 2, subject_descriptor: 3, subject_binding: 'ScalarPropertyValue', role: 'value', property_name: 'Date', label: 'Date', selected: 4, implementation: 5, usage: 6, governing_record: 6, reason: 'PersonalPreference', configuration: { DateFormat: { StringValue: 'long' } }, required_operators: [], offered_operators: [] }] } };
  invoke.mockImplementation(async (_name, { request }) => ({ request_id: request.request_id, result: { Ok: { ResolvedComposition: response } } }));
  const tx = createMapTransaction(41);
  const result = await tx.resolveComposition(createHolonReference(41,ref(1)),createHolonReference(41,ref(3)),createHolonReference(41,ref(10)));
  expect(invoke).toHaveBeenCalledOnce();
  expect(result.populations[0].members).toEqual([]);
  expect(result.work).toEqual({ selections: 1, bindingReuses: 0, metadataReuses: 2, personalPopulationRequests: 1, candidatePopulationRequests: 0 });
  expect(result.bindings[0].selected.equals(createHolonReference(41,ref(5)))).toBe(true);
  expect(result.bindings[0].usage?.equals(createHolonReference(41,ref(7)))).toBe(true);
  expect(result.bindings[0].configuration).toEqual({ DateFormat: { StringValue: 'long' } });
  expect(result.metadata[5].versionedKey).toBe('Definition5-v2');
  expect(invoke.mock.calls[0][1].request.command.Transaction.action.ResolveComposition).toEqual({ reconsider: false, owner: ref(1), subject: ref(3), theme: ref(10), demand_slot: null, collection_source: null });
  invoke.mockClear();
  await tx.resolveComposition(createHolonReference(41,ref(1)),createHolonReference(41,ref(3)),createHolonReference(41,ref(10)), undefined, undefined, { reconsider: true });
  expect(invoke).toHaveBeenCalledOnce();
  expect(invoke.mock.calls[0][1].request.command.Transaction.action.ResolveComposition.reconsider).toBe(true);
  invoke.mockClear();
  const bound = (id: number) => createHolonReference(41, ref(id));
  await tx.resolvePropertyComposition(createPropertyDescriptorHandle(bound(3)), bound(1), bound(2), bound(10));
  expect(invoke).toHaveBeenCalledOnce();
  expect(invoke.mock.calls[0][1].request.command.Transaction.action.ResolveComposition).toEqual({ reconsider: false, owner: ref(1), subject: ref(3), theme: ref(10), demand_slot: ref(2), collection_source: null });
  invoke.mockClear();
  response.data.bindings[0] = { ...response.data.bindings[0], ...{ value_holder: 0 } };
  const contextual = await tx.resolvePropertyComposition(createPropertyDescriptorHandle(bound(3)), bound(1), bound(2), bound(10), bound(1));
  expect(invoke.mock.calls[0][1].request.command.Transaction.action.ResolveComposition.value_holder).toEqual(ref(1));
  expect(contextual.bindings[0].valueHolder?.equals(bound(1))).toBe(true);
  invoke.mockClear();
  await tx.resolveRelationshipComposition(bound(1), bound(3), bound(10), bound(2), createRelationshipDescriptorHandle(bound(4)));
  expect(invoke).toHaveBeenCalledOnce();
  expect(invoke.mock.calls[0][1].request.command.Transaction.action.ResolveComposition).toEqual({ reconsider: false, owner: ref(1), subject: ref(3), theme: ref(10), demand_slot: ref(2), collection_source: ref(4) });
});

it('reads collection columns without requesting a HolonType schema for a descriptor-family anchor', async () => {
  invoke.mockImplementation(async (_name, { request }) => ({ request_id: request.request_id,
    result: { Ok: { Collection: { state: 'Fetched', members: [], keyed_index: {} } } } }));
  const collection = new DescribedHolonCollection(41, { element_type: ref(3), members: { state: 'Fetched', members: [ref(4)], keyed_index: {} } });
  expect(await collection.propertyDescriptors()).toEqual([]);
  expect(invoke).toHaveBeenCalledOnce();
  expect(invoke.mock.calls[0][1].request.command.Holon).toMatchObject({ target: ref(3), action: { Read: 'GetCollectionElementProperties' } });
});
