/** Two frame callbacks leave a rendering opportunity between reservation and content.
 * This is ordering, not a minimum animation duration (also used with reduced motion).
 */
export function destinationPaint(): Promise<void> {
  return new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
}
