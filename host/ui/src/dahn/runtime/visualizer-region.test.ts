import { describe, expect, it, vi } from 'vitest';
import { renderVisualizerRegion } from './visualizer-region';

describe('visualizer region failure boundary', () => {
  it('retains the field label and diagnostic while healthy siblings render', async () => {
    const diagnostic = vi.spyOn(console, 'error').mockImplementation(() => {});
    const failure = new Error('MissingDescribedBy');
    const bad = await renderVisualizerRegion('DisplayName', async () => { throw failure; });
    const good = await renderVisualizerRegion('Description', async () => {
      const element = document.createElement('span');
      element.textContent = 'A healthy description';
      return element;
    });
    const pane = document.createElement('section');
    pane.append(bad, good);
    expect(bad.dataset['dahnRegionState']).toBe('unavailable');
    expect(bad.textContent).toBe('DisplayName — unavailable');
    expect(bad.title).toBe('MissingDescribedBy');
    expect(pane.textContent).toContain('A healthy description');
    expect(diagnostic).toHaveBeenCalledWith('[DAHN] DisplayName unavailable', failure);
    diagnostic.mockRestore();
  });

  it('contains a child failure without replacing its successful parent', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    const parent = await renderVisualizerRegion('Properties', async () => {
      const section = document.createElement('section');
      section.append(await renderVisualizerRegion('Value', async () => {
        throw new Error('render failed');
      }));
      return section;
    });
    expect(parent.tagName).toBe('SECTION');
    expect(parent.dataset['dahnRegionState']).toBeUndefined();
    expect(parent.firstElementChild?.getAttribute('data-dahn-region-state')).toBe('unavailable');
    vi.restoreAllMocks();
  });
});
