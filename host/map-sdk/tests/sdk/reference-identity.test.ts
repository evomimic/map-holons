import { expect, it } from 'vitest';
import { createHolonReference } from '../../src/sdk/references';
import { createMapTransaction } from '../../src/sdk/transaction';

it('identifies transient and staged occurrences without requiring persistent HolonIds', () => {
  const transient = createHolonReference(41, { Transient: { tx_id: 41, id: 'same' } });
  const duplicate = createHolonReference(41, { Transient: { tx_id: 41, id: 'same' } });
  const staged = createHolonReference(41, { Staged: { tx_id: 41, id: 'same' } });
  const otherContext = createHolonReference(42, { Transient: { tx_id: 42, id: 'same' } });
  expect(transient.equals(duplicate)).toBe(true);
  expect(transient.equals(staged)).toBe(false);
  expect(transient.equals(otherContext)).toBe(false);
  expect(createMapTransaction(41).owns(transient)).toBe(true);
  expect(createMapTransaction(42).owns(transient)).toBe(false);
});

it('keeps saved identity independent of read context and cached properties', () => {
  const first = createHolonReference(41, { Smart: { holon_id: { Local: [1, 2] }, smart_property_values: null } });
  const second = createHolonReference(42, { Smart: { holon_id: { Local: [1, 2] }, smart_property_values: { Key: { StringValue: 'cached' } } } });
  expect(first.equals(second)).toBe(true);
});

it('compares external identities structurally and includes the Space', () => {
  const first = createHolonReference(41, { Smart: { holon_id: { External: { space_id: [10], local_id: [1] } }, smart_property_values: null } });
  const duplicate = createHolonReference(42, { Smart: { holon_id: { External: { local_id: [1], space_id: [10] } }, smart_property_values: null } });
  const otherSpace = createHolonReference(41, { Smart: { holon_id: { External: { space_id: [20], local_id: [1] } }, smart_property_values: null } });
  expect(first.equals(duplicate)).toBe(true);
  expect(first.equals(otherSpace)).toBe(false);
});
