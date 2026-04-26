import { describe, expect, it } from 'vitest';
import { readText } from './test-helpers';

describe('DAHN dependency seam', () => {
  it('defines a curated DAHN-local bridge to the public MAP SDK', () => {
    const seamSource = readText('deps/map-sdk.ts');

    expect(seamSource).toContain('public MAP SDK surface');
    expect(seamSource).toContain("from '../../../../map-sdk/src'");
    expect(seamSource).not.toContain('map-sdk/src/internal/');
  });

  it('re-exports the public SDK bridge through the local deps barrel', () => {
    const depsIndexSource = readText('deps/index.ts');

    expect(depsIndexSource).toContain("from './map-sdk'");
  });

  it('uses the local deps barrel from DAHN contracts and contract checks', () => {
    expect(readText('contracts/targets.ts')).toContain("from '../deps'");
    expect(readText('contracts/holon-view.ts')).toContain("from '../deps'");
    expect(readText('contract-checks.ts')).toContain("from './deps'");
  });
});
