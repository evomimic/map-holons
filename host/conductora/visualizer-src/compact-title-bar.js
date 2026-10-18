import { layoutTitleIdentity } from './node-title-bar.js';

/** Independent compact title realization: compact identity disclosure. */
export default class CompactTitleBar extends HTMLElement {
  get visualizerOperations() { return { 'ApplyTitlePresentation.VisualizerOperator': input => this.apply(input) }; }
  setContext(context) {
    this.dataset.nodeTitleBar = 'true';
    this.setAttribute('role', 'banner');
    this.identity = document.createElement('button'); this.identity.type = 'button';
    this.identity.style.cssText = 'min-width:0;flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font:inherit;color:inherit;background:transparent;border:0;';
    this.style.cssText = 'display:flex;min-width:0;min-height:0;overflow:auto;box-sizing:border-box;background:var(--dahn-action-surface-background);color:var(--dahn-action-text-color);';
    this.identity.addEventListener('click', () => { this.invoke('RestoreNodePresentation.VisualizerOperator', this.identity); this.identity.textContent = this.input.title; });
    this.tabIndex = -1; this.replaceChildren(this.identity); this.ready = Promise.resolve();
  }
  setOwnerInvocation(invoke) { this.invoke = invoke; }
  apply(input) {
    if (!Number.isFinite(input.width) || !Number.isFinite(input.height) || input.width < 1 || input.height < 32 || !input.title) throw new Error('Invalid compact title grant');
    this.input = input; this.identity.textContent = input.holonKey || input.title;
    this.identity.setAttribute('aria-label', `${input.requiresRestore ? 'Restore: ' : ''}${input.title}`); this.identity.title = input.title;
    layoutTitleIdentity(this, this.identity, input);
    return { status: 'applied' };
  }
  getVisualizerComposition() { return []; }
}
