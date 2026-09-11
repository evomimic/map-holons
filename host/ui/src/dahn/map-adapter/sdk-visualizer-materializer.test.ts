import { describe, expect, it, vi } from 'vitest';
import { SdkVisualizerMaterializer } from './sdk-visualizer-materializer';

describe('SdkVisualizerMaterializer', () => {
  it('materializes through the public SDK before consuming its artifact capability', async () => {
    const materializeVisualizer = vi.fn().mockResolvedValue({
      artifactHandle: 'artifact:one',
      format: 'ESModule',
      entrypoint: 'default',
    });
    const fetchArtifact = vi.fn().mockResolvedValue(
      new TextEncoder().encode('export default class Visualizer {}'),
    );
    const adapter = new SdkVisualizerMaterializer({
      materializeVisualizer,
      fetchArtifact,
    } as never);
    const selected = {};

    await expect(adapter.materialize(selected as never)).resolves.toEqual({
      source: 'export default class Visualizer {}',
      format: 'ESModule',
      entrypoint: 'default',
    });
    expect(materializeVisualizer).toHaveBeenCalledWith(selected);
    expect(fetchArtifact).toHaveBeenCalledWith('artifact:one');
    expect(materializeVisualizer.mock.invocationCallOrder[0]).toBeLessThan(
      fetchArtifact.mock.invocationCallOrder[0],
    );
  });

  it('does not consume an artifact whose declared module format is unsupported', async () => {
    const fetchArtifact = vi.fn();
    const adapter = new SdkVisualizerMaterializer({
      materializeVisualizer: vi.fn().mockResolvedValue({
        artifactHandle: 'artifact:one',
        format: 'CommonJS',
        entrypoint: 'default',
      }),
      fetchArtifact,
    } as never);

    await expect(adapter.materialize({} as never)).rejects.toThrow('Unsupported');
    expect(fetchArtifact).not.toHaveBeenCalled();
  });
});
