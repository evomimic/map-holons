/** Path-owned participation policy. Content and retained band counts are not inputs. */
export const APPLY_NODE_PRESENTATION = 'ApplyNodePresentationV2.VisualizerOperator';
export const PRESENTATION_REQUESTS = ['inspect', 'collection-context', 'relationship', 'compact'];
/** Theme font size scales the tested default profile; each owner passes its own inputs. */
export function participationProfile(fontSize = 16) {
  const scale = Math.max(1, fontSize / 16);
  const axis = (minimum, preferred, channel) => ({ minimum: minimum.map(n => n * scale), preferred: preferred.map(n => n * scale), channel: { minimum: channel[0] * scale, preferred: channel[1] * scale } });
  return { horizontal: axis([80, 180, 360], [96, 240, 800], [48, 144]), vertical: axis([64, 320, 480], [64, 400, 720], [24, 64]) };
}
export function resolveAxis(profile, budget, frame = 0) {
  const { minimum, preferred, channel } = profile;
  if ([...minimum, ...preferred, channel.minimum, channel.preferred].some(n => !Number.isFinite(n) || n <= 0)
      || minimum.some((n, i) => n > preferred[i] || i && n < minimum[i - 1])
      || preferred.some((n, i) => i && n < preferred[i - 1]) || channel.minimum > channel.preferred) throw new Error('Invalid Node participation profile');
  const M = minimum.reduce((a, b) => a + b, 0) + 2 * channel.minimum;
  const P = preferred.reduce((a, b) => a + b, 0) + 2 * channel.preferred;
  const f = P === M ? 1 : Math.max(0, Math.min(1, (budget - frame - M) / (P - M)));
  return { compact: Math.ceil(minimum[0] + f * (preferred[0] - minimum[0])), partial: Math.ceil(minimum[1] + f * (preferred[1] - minimum[1])), expanded: Math.ceil(minimum[2] + f * (preferred[2] - minimum[2])), channel: Math.ceil(channel.minimum + f * (channel.preferred - channel.minimum)) };
}
export function validateNodePresentation(input, mode = 'properties') {
  if (!input || 'requiredCapabilities' in input || (input.presentation !== undefined && !PRESENTATION_REQUESTS.includes(input.presentation))) throw new Error('Invalid Node presentation assignment');
  const effective = input.presentation === 'compact' || input.presentation === 'relationship' ? input.presentation
    : input.presentation ? 'properties' : mode;
  const profile = participationProfile();
  const minimum = { width: profile.horizontal.minimum[effective === 'compact' ? 0 : effective === 'relationship' ? 1 : 2],
    height: profile.vertical.minimum[effective === 'compact' ? 0 : 1] };
  if (!Number.isInteger(input.width) || !Number.isInteger(input.height) || input.width < minimum.width || input.height < minimum.height) throw new Error('Node presentation is outside the supported participation range');
  return { width: input.width, height: input.height, ...(input.presentation === undefined ? {} : { presentation: input.presentation }) };
}
/** Invoke only the explicitly admitted receiver; no private-name or renderer fallback. */
// Shared stable symbol lets the authored runtime and bundled artifacts read the
// same last owner assignment during atomic Visualizer replacement.
const accepted = Symbol.for('map.dahn.node-presentation-v2');
/** Retained owner assignment for atomic replacement; this is not child mode discovery. */
export function assignedNodePresentation(element) {
  const input = element[accepted];
  return input && { ...input };
}
export function applyNodePresentation(element, input) {
  const binding = element.visualizerOperations?.[APPLY_NODE_PRESENTATION];
  if (typeof binding !== 'function') throw new Error('Missing ApplyNodePresentation binding');
  validateNodePresentation(input, input.presentation === undefined ? 'compact' : undefined);
  const prior = element[accepted];
  try {
    const result = binding(input);
    if (!result || result.status !== 'applied') throw new Error(result?.reason || 'Node refused its contracted presentation');
    element[accepted] = { ...input, presentation: input.presentation ?? prior?.presentation ?? 'inspect' };
    return result;
  } catch (error) {
    if (prior) { try { binding(prior); } catch { /* The owner exposes recovery for the violated contract. */ } }
    throw error;
  }
}
