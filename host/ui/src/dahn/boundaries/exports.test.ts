import { describe, expect, it } from 'vitest';
import * as dahn from '../index';

describe('DAHN export surface', () => {
  it('exports the expected Wave 0 DAHN runtime seams', () => {
    expect(dahn.DefaultVisualizerRegistry).toBeDefined();
    expect(dahn.DomCanvas).toBeDefined();
    expect(dahn.MaterializedVisualizerCache).toBeDefined();
    expect(dahn.MaterializedVisualizerRuntime).toBeDefined();
    expect(dahn.Theme).toBeDefined();
    expect(dahn.HolonSpaceThemeResolver).toBeDefined();
    expect(dahn.DahnHolonView).toBeDefined();
  });

  it('does not leak obvious transport-facing concepts through the DAHN root export surface', () => {
    const exportNames = Object.keys(dahn);

    expect(exportNames).not.toContain('MapIpcRequest');
    expect(exportNames).not.toContain('MapIpcResponse');
    expect(exportNames).not.toContain('RequestOptions');
    expect(exportNames.every((name) => !name.endsWith('Wire'))).toBe(true);
    expect(exportNames.every((name) => !name.includes('Internal'))).toBe(true);
  });
});
