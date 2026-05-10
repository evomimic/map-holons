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

  it('uses MAP SDK holon handles instead of declaring DAHN-local descriptor handles', () => {
    const holonViewSource = readText('contracts/holon-view.ts');
    const actionsSource = readText('contracts/actions.ts');
    const dahnIndexSource = readText('index.ts');

    expect(holonViewSource).toContain(
      'export type HolonViewAccess = HolonReference',
    );
    expect(holonViewSource).not.toContain('interface ValueTypeDescriptorHandle');
    expect(holonViewSource).not.toContain('interface PropertyDescriptorHandle');
    expect(holonViewSource).not.toContain(
      'interface RelationshipDescriptorHandle',
    );
    expect(holonViewSource).not.toContain('interface DanceDescriptorHandle');
    expect(holonViewSource).not.toContain('interface HolonTypeDescriptorHandle');
    expect(actionsSource).toContain('dance?: HolonReference;');
    expect(dahnIndexSource).not.toContain('DanceDescriptorHandle');
    expect(dahnIndexSource).not.toContain('HolonTypeDescriptorHandle');
  });
});
