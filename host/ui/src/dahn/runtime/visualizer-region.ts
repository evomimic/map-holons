/** Retains an unavailable region's diagnostic without substituting a visualizer. */
export function unavailableVisualizerRegion(label: string, error: unknown): HTMLElement {
  console.error(`[DAHN] ${label} unavailable`, error);
  const region = document.createElement('div');
  region.dataset['dahnRegionState'] = 'unavailable';
  region.setAttribute('role', 'status');
  region.textContent = `${label} — unavailable`;
  region.title = error instanceof Error ? error.message : String(error);
  return region;
}

/** Contains construction, selection, data-read, and rendering failures to one region. */
export async function renderVisualizerRegion(
  label: string,
  render: () => Promise<HTMLElement>,
): Promise<HTMLElement> {
  try {
    return await render();
  } catch (error) {
    return unavailableVisualizerRegion(label, error);
  }
}
