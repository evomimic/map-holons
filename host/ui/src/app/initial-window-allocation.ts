import { isTauri } from '@tauri-apps/api/core';
import type { SpatialExtents } from '../dahn/contracts/visualizers';

/** The application alone negotiates native size, once after initial composition. */
export async function allocateInitialWindow(report: SpatialExtents | undefined): Promise<void> {
  if (!report || !isTauri()) return;
  const { getCurrentWindow, currentMonitor, LogicalSize } = await import('@tauri-apps/api/window');
  const window = getCurrentWindow();
  const [inner, outer, scale, monitor] = await Promise.all([
    window.innerSize(), window.outerSize(), window.scaleFactor(), currentMonitor(),
  ]);
  const current = inner.toLogical(scale);
  const available = monitor?.workArea.size.toLogical(scale);
  const decorations = { width: (outer.width - inner.width) / scale, height: (outer.height - inner.height) / scale };
  const required = report.preferred ?? report.minimum;
  const width = Math.min(Math.max(current.width, required.width), available ? available.width - decorations.width : Infinity);
  const height = Math.min(Math.max(current.height, required.height), available ? available.height - decorations.height : Infinity);
  if (width !== current.width || height !== current.height) await window.setSize(new LogicalSize(Math.ceil(width), Math.ceil(height)));
}
