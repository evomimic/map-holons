import { participationProfile } from '../../conductora/visualizer-src/node-presentation.js';
/** Translate historical test vectors into the canonical input (no production axis protocol). */
export function presentationAssignment(input: any) {
  const state = (axis: string) => axis?.startsWith('full') ? 'expanded' : axis?.startsWith('partial') ? 'partial' : 'compact';
  const horizontal = state(input.horizontal), vertical = state(input.vertical);
  const presentation = input.presentation ?? (horizontal === 'compact' || vertical === 'compact' ? 'compact'
    : horizontal === 'partial' ? 'relationship' : vertical === 'partial' ? 'collection-context' : 'inspect');
  return { width: input.width, height: input.height, presentation };

}
/** Declared policy data for fixtures; never a report from a selected child. */
export function profileExtents() {
  const profile = participationProfile();
  return { horizontal: { 'minimal-width': profile.horizontal.preferred[0], 'partial-width': profile.horizontal.preferred[1], 'full-width': profile.horizontal.preferred[2] },
    vertical: { 'minimal-height': profile.vertical.preferred[0], 'partial-height': profile.vertical.preferred[1], 'full-height': profile.vertical.preferred[2] } };
}
