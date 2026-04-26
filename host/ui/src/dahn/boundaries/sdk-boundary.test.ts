import { describe, expect, it } from 'vitest';
import { relative } from 'node:path';
import { dahnRoot, listDahnSourceFiles, readText } from './test-helpers';

describe('DAHN SDK boundary', () => {
  it('does not import MAP SDK internals anywhere in DAHN', () => {
    for (const file of listDahnSourceFiles()) {
      const relativePath = relative(dahnRoot(), file);

      if (relativePath.startsWith('boundaries/')) {
        continue;
      }

      const source = readText(relativePath);
      expect(source).not.toContain('map-sdk/src/internal/');
      expect(source).not.toContain("from '../../../../map-sdk/src/internal");
      expect(source).not.toContain("from '../../../map-sdk/src/internal");
    }
  });

  it('routes public SDK imports only through the DAHN-local dependency seam', () => {
    for (const file of listDahnSourceFiles()) {
      const relativePath = relative(dahnRoot(), file);

      if (
        relativePath === 'deps/map-sdk.ts' ||
        relativePath.startsWith('boundaries/')
      ) {
        continue;
      }

      const source = readText(relativePath);
      expect(source).not.toContain('map-sdk/src');
    }
  });
});
