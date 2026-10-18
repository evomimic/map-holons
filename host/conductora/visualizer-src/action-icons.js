/** Stable vector artwork for the standard occurrence actions, independent of fonts. */
export function actionIcon(operator, restore = false) {
  const paths = {
    'ExploreOccurrence.VisualizerOperator': 'M8 5H5v14h14v-3 M12 5h7v7 M19 5L9 15',
    'CloseOccurrence.VisualizerOperator': 'M6 6l12 12 M18 6L6 18',
    'ExpandRestoreOccurrence.VisualizerOperator': restore
      ? 'M19 5l-6 6m0-5v5h5 M5 19l6-6m-5 0h5v5'
      : 'M14 5h5v5 M19 5l-6 6 M10 19H5v-5 M5 19l6-6',
    'InspectOccurrence.VisualizerOperator': 'M9 9l3 6 3-6',
  };
  if (!paths[operator]) return undefined;
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.setAttribute('viewBox', '0 0 24 24'); svg.setAttribute('width', '18'); svg.setAttribute('height', '18');
  svg.setAttribute('aria-hidden', 'true'); svg.setAttribute('focusable', 'false');
  svg.style.cssText = 'display:block;flex:none;';
  if (operator === 'InspectOccurrence.VisualizerOperator') {
    const circle = document.createElementNS(svg.namespaceURI, 'circle');
    circle.setAttribute('cx', '12'); circle.setAttribute('cy', '12'); circle.setAttribute('r', '9');
    circle.setAttribute('fill', 'none'); circle.setAttribute('stroke', 'currentColor'); svg.append(circle);
  }
  const path = document.createElementNS(svg.namespaceURI, 'path');
  path.setAttribute('d', paths[operator]); path.setAttribute('fill', 'none'); path.setAttribute('stroke', 'currentColor');
  path.setAttribute('stroke-width', '1.8'); path.setAttribute('stroke-linecap', 'round'); path.setAttribute('stroke-linejoin', 'round');
  svg.append(path); return svg;
}
