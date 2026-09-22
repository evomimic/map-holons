import { beforeEach, expect, it, vi } from 'vitest';
import { createHolonReference } from '../../src/sdk/references';
import { MapClient } from '../../src/sdk/client';
import { isMapCommandWire } from '../../src/internal/wire-types/commands';
import type { MapIpcRequest, MapResultWire } from '../../src/internal/wire-types';
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
const reference = (id: string) => ({ Transient: { tx_id: 41, id } });
const ownerWire = reference('2f9dcd83-47ee-482e-8059-28dca43d8a64');
const descriptorWire = reference('fcb56a31-c1cb-4066-b4c3-d185809c2864');
const memberWire = reference('11111111-1111-4111-8111-111111111111');
const calls: MapIpcRequest[] = [];
let count = 0;
beforeEach(() => {
  calls.length = 0; count = 0;
  invoke.mockImplementation(async (_command, { request }: { request: MapIpcRequest }) => {
    calls.push(request); expect(isMapCommandWire(request.command)).toBe(true);
    let result: MapResultWire = 'None';
    if ('Space' in request.command) result = { TransactionCreated: { tx_id: 41 } };
    if ('Holon' in request.command) {
      const action = request.command.Holon.action;
      if ('Read' in action) {
        const read = action.Read;
        if (typeof read === 'object' && 'GetDescribedRelatedHolons' in read) result = { DescribedCollection: { members: { state: 'Fetched', members: Array.from({ length: count }, () => memberWire), keyed_index: {} }, element_type: descriptorWire } };
        if (read === 'GetInstanceProperties') result = { Collection: { state: 'Fetched', members: [descriptorWire], keyed_index: {} } };
        if (read === 'GetPropertyValueKind') result = { Value: { StringValue: 'AnyBaseValue' } };
        if (typeof read === 'object' && 'GetPropertyValue' in read) result = { Value: { StringValue: read.GetPropertyValue.name === 'DisplayName' ? 'Default Value' : 'DefaultValue' } };
        if (typeof read === 'object' && 'GetPropertyValue' in read && read.GetPropertyValue.name === 'DefaultValue') result = { Value: { IntegerValue: 7 } };
      }
    }
    if ('Transaction' in request.command) result = { VisualizerSelection: { selected: descriptorWire, requested_kind: 'Collection', alternatives_available: false } };
    return { request_id: request.request_id, result: { Ok: result } };
  });
});
it.each([0, 1, 3])('carries %i members and their declared type through the actual SDK transport and selector binding', async size => {
  count = size;
  const transaction = await new MapClient().beginTransaction();
  const owner = createHolonReference(41, ownerWire);
  const collection = await owner.describedRelatedHolons('InverseName');
  expect(collection.length).toBe(size);
  const [property] = await collection.elementType.instanceProperties();
  expect(await property.displayName()).toBe('Default Value');
  expect(await property.valueKind()).toBe('AnyBaseValue');
  if (size) expect(await collection.members[0].propertyValue('DefaultValue')).toEqual({ IntegerValue: 7 });
  const selection = await transaction.selectCollectionVisualizer(collection, owner, owner);
  expect(selection.requestedKind).toBe('collection');
  const last = calls.at(-1)!;
  expect(last.command).toMatchObject({ Transaction: { tx_id: 41, action: { SelectCollectionVisualizer: { collection: { element_type: descriptorWire, members: { members: Array(size).fill(memberWire) } }, parent_visualizer: ownerWire, slot: ownerWire } } } });
  expect(calls[1].command).toMatchObject({ Holon: { target: ownerWire, action: { Read: { GetDescribedRelatedHolons: { name: 'InverseName' } } } } });
});
it('rejects a described collection with missing element metadata at transport ingress', async () => {
  invoke.mockImplementation(async (_command, { request }) => ({ request_id: request.request_id, result: { Ok: { DescribedCollection: { members: { state: 'Fetched', members: [], keyed_index: {} } } } } }));
  await expect(createHolonReference(41, ownerWire).describedRelatedHolons('Children')).rejects.toThrow();
});
