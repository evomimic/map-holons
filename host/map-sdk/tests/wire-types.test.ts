import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import {
  isMapIpcRequest,
  isMapIpcResponse,
} from '../src/internal/wire-types/index';

const fixturesDir = join(dirname(fileURLToPath(import.meta.url)), 'fixtures');
const fixtureFiles = readdirSync(fixturesDir).sort();

describe('wire type fixtures', () => {
  it('discovers the generated fixture set', () => {
    expect(fixtureFiles.length).toBe(41);
  });

  for (const fixtureFile of fixtureFiles) {
    it(`validates ${fixtureFile}`, () => {
      const fixturePath = join(fixturesDir, fixtureFile);
      const payload = JSON.parse(readFileSync(fixturePath, 'utf8')) as unknown;

      const isValid =
        fixtureFile.startsWith('request-')
          ? isMapIpcRequest(payload)
          : isMapIpcResponse(payload);

      expect(isValid).toBe(true);
    });
  }
});
