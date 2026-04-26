import { describe, expect, it } from 'vitest';
import * as dahn from '../index';

describe('DAHN export surface', () => {
  it('exports the expected Wave 0 DAHN runtime seams', () => {
    expect(dahn.DefaultVisualizerRegistry).toBeDefined();
    expect(dahn.DomCanvas).toBeDefined();
    expect(dahn.Phase0Selector).toBeDefined();
    expect(dahn.DEFAULT_DAHN_THEME).toBeDefined();
    expect(dahn.DefaultThemeRegistry).toBeDefined();
    expect(dahn.registerBuiltInVisualizers).toBeDefined();
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
