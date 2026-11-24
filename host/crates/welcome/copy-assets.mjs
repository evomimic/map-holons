// Angular restricts asset inputs to its UI workspace. Publish Welcome's own
// assets after that build, retaining the same packaged URLs and one source copy.
import { cp } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
await cp(fileURLToPath(new URL('./public/', import.meta.url)),
  fileURLToPath(new URL('../../ui/dist/map_sdk/browser/', import.meta.url)),
  { recursive: true });
