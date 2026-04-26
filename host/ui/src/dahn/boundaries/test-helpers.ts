import { readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, extname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const THIS_DIR = dirname(fileURLToPath(import.meta.url));
const DAHN_ROOT = join(THIS_DIR, '..');

export function dahnRoot(): string {
  return DAHN_ROOT;
}

export function readText(relativePath: string): string {
  return readFileSync(join(DAHN_ROOT, relativePath), 'utf8');
}

export function listDahnSourceFiles(): string[] {
  const files: string[] = [];

  function walk(directory: string): void {
    for (const entry of readdirSync(directory)) {
      const fullPath = join(directory, entry);
      const stats = statSync(fullPath);

      if (stats.isDirectory()) {
        walk(fullPath);
        continue;
      }

      if (extname(fullPath) === '.ts') {
        files.push(fullPath);
      }
    }
  }

  walk(DAHN_ROOT);
  return files;
}
