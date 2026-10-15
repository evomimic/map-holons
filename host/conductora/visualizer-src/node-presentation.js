/** Path-owned participation policy. Content and retained band counts are not inputs. */
export const APPLY_NODE_PRESENTATION = 'ApplyNodePresentation.VisualizerOperator';
export const CAPABILITIES = ['inspect', 'navigate-singular', 'navigate-plural', 'present-active-collection'];
const canonical = [[], ['inspect'], ['navigate-singular'], ['navigate-plural', 'present-active-collection'], ['navigate-singular', 'navigate-plural', 'present-active-collection']];
export function nodeCapabilities(horizontal, vertical) {
  if (horizontal === 'compact' || vertical === 'compact') return [];
  if (horizontal === 'expanded' && vertical === 'expanded') return ['inspect'];
  return [...(horizontal === 'partial' ? ['navigate-singular'] : []), ...(vertical === 'partial' ? ['navigate-plural', 'present-active-collection'] : [])];
}
/** Theme font size scales the tested default profile; each owner passes its own inputs. */
export function participationProfile(fontSize = 16) {
  const scale = Math.max(1, fontSize / 16);
  const axis = (minimum, preferred, channel) => ({ minimum: minimum.map(n => n * scale), preferred: preferred.map(n => n * scale), channel: { minimum: channel[0] * scale, preferred: channel[1] * scale } });
  return { horizontal: axis([80, 180, 360], [96, 240, 800], [48, 144]), vertical: axis([64, 200, 320], [64, 300, 640], [24, 64]) };
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
export function validateNodePresentation(input) {
  const caps = input?.requiredCapabilities;
  if (!Array.isArray(caps) || new Set(caps).size !== caps.length
      || !canonical.some(set => set.length === caps.length && set.every(c => caps.includes(c)))) throw new Error('Invalid Node capabilities');
  const inspect = caps.includes('inspect'), plural = caps.includes('navigate-plural'), singular = caps.includes('navigate-singular');
  const profile = participationProfile();
  const minimum = { width: profile.horizontal.minimum[inspect || plural && !singular ? 2 : singular ? 1 : 0], height: profile.vertical.minimum[inspect || singular && !plural ? 2 : plural ? 1 : 0] };
  if (!Number.isFinite(input.width) || !Number.isFinite(input.height) || input.width < minimum.width || input.height < minimum.height) throw new Error('Node presentation is outside the supported participation range');
  return { width: input.width, height: input.height, requiredCapabilities: [...caps] };
}
/** Invoke only the explicitly admitted receiver; no private-name or renderer fallback. */
const accepted = new WeakMap();
export function applyNodePresentation(element, input) {
  const binding = element.visualizerOperations?.[APPLY_NODE_PRESENTATION];
  if (typeof binding !== 'function') throw new Error('Missing ApplyNodePresentation binding');
  validateNodePresentation(input);
  const prior = accepted.get(element);
  try {
    const result = binding(input);
    if (!result || result.status !== 'applied') throw new Error(result?.reason || 'Node refused its contracted presentation');
    accepted.set(element, input);
    return result;
  } catch (error) {
    if (prior) { try { binding(prior); } catch { /* The owner exposes recovery for the violated contract. */ } }
    throw error;
  }
}
