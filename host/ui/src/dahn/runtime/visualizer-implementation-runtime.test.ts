import { describe, expect, it } from 'vitest';
import type {
  HolonReference,
  VisualizerImplementationResolution,
} from '../index';
import { registerBuiltInVisualizers } from '../registry/register-builtins';
import { DefaultVisualizerRegistry } from '../registry/default-visualizer-registry';
import {
  DefaultVisualizerImplementationRuntime,
} from './visualizer-implementation-runtime';
import { DahnImplementationResolutionError } from './runtime-errors';

const selectedVisualizer = {} as HolonReference;
const implementation = {} as HolonReference;

function resolution(
  overrides: Partial<VisualizerImplementationResolution> = {},
): VisualizerImplementationResolution {
  return {
    selectedVisualizer,
    implementation,
    runtime: 'TypeScript',
    implementationKey: 'dahn.generic-holon-node',
    ...overrides,
  };
}

function runtime(): DefaultVisualizerImplementationRuntime {
  const registry = new DefaultVisualizerRegistry();
  registerBuiltInVisualizers(registry);
  return new DefaultVisualizerImplementationRuntime(registry);
}

describe('DefaultVisualizerImplementationRuntime', () => {
  it.each([
    ['dahn.space-navigator', 'space-navigator'],
    ['dahn.generic-holon-node', 'holon-node'],
    ['dahn.table-collection', 'table-collection'],
  ])('resolves %s to the local %s executable definition', async (key, id) => {
    await expect(runtime().resolve(resolution({ implementationKey: key }))).resolves
      .toMatchObject({ id, implementationKey: key });
  });

  it('does not select a fallback when the supplied implementation key is unavailable', async () => {
    const input = resolution({ implementationKey: 'dahn.unavailable' });

    await expect(runtime().resolve(input)).rejects.toMatchObject({
      name: 'DahnImplementationResolutionError',
      reason: 'unknown-implementation-key',
      selectedVisualizer,
      implementation,
      implementationKey: 'dahn.unavailable',
    } satisfies Partial<DahnImplementationResolutionError>);
  });

  it('rejects an implementation for a different runtime', async () => {
    await expect(runtime().resolve(resolution({ runtime: 'Rust' }))).rejects
      .toMatchObject({ reason: 'unsupported-runtime', runtime: 'Rust' });
  });

  it('rejects a missing implementation key with diagnostic identity context', async () => {
    const input = resolution({ implementationKey: undefined });

    await expect(runtime().resolve(input)).rejects.toMatchObject({
      reason: 'missing-implementation-key',
      selectedVisualizer,
      implementation,
    });
  });

  it('preserves the selected identity when local definition loading fails', async () => {
    const registry = new DefaultVisualizerRegistry();
    registry.register({
      id: 'broken-local-definition',
      implementationKey: 'dahn.broken',
      displayName: 'Broken local definition',
      version: '0.0.0',
      componentTag: 'test-broken-local-definition',
      supportedTargets: [{ kind: 'canvas' }],
      load: async () => {
        throw new Error('load failed');
      },
    });
    const input = resolution({ implementationKey: 'dahn.broken' });

    await expect(
      new DefaultVisualizerImplementationRuntime(registry).resolve(input),
    ).rejects.toMatchObject({
      reason: 'load-failed',
      selectedVisualizer,
      implementation,
      implementationKey: 'dahn.broken',
    });
  });
});
