import type { SpatialExtents } from '../contracts/visualizers';

/** Add only this owner's measured framing to its immediate child's report. */
export function composedSpatialExtents(owner: HTMLElement, child: HTMLElement, report: SpatialExtents): SpatialExtents {
  const width = Math.max(0, owner.offsetWidth - child.offsetWidth);
  const height = Math.max(0, owner.offsetHeight - child.offsetHeight);
  const add = (extent: { width: number; height: number }) => ({ width: extent.width + width, height: extent.height + height });
  return { minimum: add(report.minimum), preferred: report.preferred && add(report.preferred) };
}
