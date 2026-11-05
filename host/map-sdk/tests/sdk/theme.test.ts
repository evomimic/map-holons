import { beforeEach, expect, it, vi } from 'vitest';
import { createHolonReference } from '../../src/sdk/references';
import type { HolonReferenceWire } from '../../src/internal/wire-types';
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
const ref = (id: number): HolonReferenceWire => ({ Smart: { holon_id: { Local: [id] }, smart_property_values: null } });
beforeEach(() => vi.resetAllMocks());
function result(count: number) {
  return { theme: ref(1), meta_design_system: ref(2), tokens: Array.from({ length: count }, (_, i) => ref(i + 3)), data: {
    theme_key: 'Independent.Theme', theme_versioned_key: 'Independent.Theme-v3', meta_design_system_key: 'Independent.MDS', meta_design_system_versioned_key: 'Independent.MDS-v2',
    bindings: Array.from({ length: count }, (_, i) => ({ token_versioned_key: `Token${i}-v4`, token_name: `Token${i}`, presentation_value: '1rem' })),
  } };
}
it.each([1, 63, 126])('loads %i bindings through exactly one boundary call and retains exact references', async count => {
  const response = result(count);
  invoke.mockImplementation(async (_name, { request }) => ({ request_id: request.request_id, result: { Ok: { LoadedTheme: response } } }));
  const theme = createHolonReference(41, ref(1));
  const loaded = await theme.loadTheme();
  expect(invoke).toHaveBeenCalledTimes(1);
  expect(invoke.mock.calls[0][1].request.command).toEqual({ Transaction: { tx_id: 41, action: { LoadTheme: { theme: ref(1) } } } });
  expect(loaded.reference).toBe(theme);
  expect(loaded.metaDesignSystem.equals(createHolonReference(41, ref(2)))).toBe(true);
  expect(loaded.bindings).toHaveLength(count);
  expect(loaded.bindings[0].token.equals(createHolonReference(41, ref(3)))).toBe(true);
  expect(loaded.bindings[0].tokenVersionedKey).toBe('Token0-v4');
});
it.each(['missing value', 'reference count mismatch', 'different Theme'])('rejects malformed or mismatched projection: %s', async kind => {
  const response = result(1);
  if (kind === 'missing value') delete (response.data.bindings[0] as Partial<typeof response.data.bindings[0]>).presentation_value;
  if (kind === 'reference count mismatch') response.tokens = [];
  if (kind === 'different Theme') response.theme = ref(99);
  invoke.mockImplementation(async (_name, { request }) => ({ request_id: request.request_id, result: { Ok: { LoadedTheme: response } } }));
  await expect(createHolonReference(41, ref(1)).loadTheme()).rejects.toThrow();
  expect(invoke).toHaveBeenCalledTimes(1);
});
