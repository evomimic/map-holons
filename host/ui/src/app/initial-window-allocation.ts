import { isTauri } from '@tauri-apps/api/core';
import type { SpatialExtents } from '../dahn/contracts/visualizers';

/** Negotiate size and placement together, once after initial composition. */
export async function allocateInitialWindow(report: SpatialExtents | undefined): Promise<void> {
  if (!report || !isTauri()) return;
  const { getCurrentWindow, currentMonitor, LogicalSize, PhysicalPosition } = await import('@tauri-apps/api/window');
  const window = getCurrentWindow();
  const [inner, outer, scale, monitor] = await Promise.all([
    window.innerSize(), window.outerSize(), window.scaleFactor(), currentMonitor(),
  ]);
  const current = inner.toLogical(scale);
  // Native startup already fits the window. Without monitor bounds, keep that
  // allocation rather than growing into an unknown amount of desktop space.
  if (!monitor) return;
  const available = monitor.workArea.size.toLogical(scale);
  const decorations = { width: (outer.width - inner.width) / scale, height: (outer.height - inner.height) / scale };
  const required = report.preferred ?? report.minimum;
  const margin = 16;
  const width = Math.max(1, Math.floor(Math.min(Math.max(current.width, required.width), available.width - decorations.width - 2 * margin)));
  const height = Math.max(1, Math.floor(Math.min(Math.max(current.height, required.height), available.height - decorations.height - 2 * margin)));
  if (width !== current.width || height !== current.height) await window.setSize(new LogicalSize(width, height));
  // A resized window cannot keep the old top-left position: the initial native
  // placement was centered for a smaller rectangle, leaving less room below it.
  const outerWidth = width * scale + outer.width - inner.width;
  const outerHeight = height * scale + outer.height - inner.height;
  await window.setPosition(new PhysicalPosition(
    monitor.workArea.position.x + Math.floor((monitor.workArea.size.width - outerWidth) / 2),
    monitor.workArea.position.y + Math.floor((monitor.workArea.size.height - outerHeight) / 2),
  ));
}
