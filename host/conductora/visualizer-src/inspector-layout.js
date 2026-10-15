import { validateNodePresentation } from './node-presentation.js';

/** Immediate-slot geometry shared by generic and specialized Node bindings. */
export function inspectorLayout(input, titleHeight = 48) {
  const grant = validateNodePresentation(input);
  const has = capability => grant.requiredCapabilities.includes(capability);
  const inspect = has('inspect');
  const singular = inspect || has('navigate-singular');
  const plural = inspect || has('navigate-plural');
  const title = Math.min(grant.height, titleHeight);
  const remaining = grant.height - title;
  const body = singular ? (plural ? Math.floor(remaining * .4) : remaining) : 0;
  return { ...grant, inspect, singular, plural, title, body, collection: plural ? remaining - body : 0,
    vertical: inspect || singular && !plural ? 'full-height' : plural ? 'partial-height' : 'minimal-height',
    horizontal: inspect || plural && !singular ? 'full-width' : singular ? 'partial-width' : 'minimal-width' };
}
export function boundedRegion(element, visible, label) {
  if (!element) return;
  element.hidden = !visible; element.inert = !visible;
  Object.assign(element.style, { minWidth: '0', minHeight: '0', maxWidth: '100%', maxHeight: '100%', boxSizing: 'border-box', overflow: 'auto' });
  if (visible) { element.tabIndex = 0; element.setAttribute('aria-label', label); }
}
