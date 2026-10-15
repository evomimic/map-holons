/** Operator-backed action; it never fabricates a Dance or resolves a DOM ancestor. */
export default class PresentationAction extends HTMLElement {
  get visualizerOperations() { return { 'ApplyPresentationAction.VisualizerOperator': state => {
    this.button.disabled = !state.available;
    const label = state.label || this.context.title;
    this.button.textContent = this.glyph || label;
    this.button.title = state.available ? label : state.reason;
    this.button.setAttribute('aria-label', `${label}${state.available ? '' : `: ${state.reason}`}`);
    return { status: 'applied' };
  } }; }
  setContext(context) {
    if (typeof context.presentationActivation !== 'function') throw new Error('Presentation Action requires occurrence authority');
    this.context = context;
    this.dataset.presentationOperator = context.presentationOperator;
    this.style.cssText = 'display:inline-flex;flex:0 0 auto;min-width:0;max-width:100%;';
    this.button = document.createElement('button'); this.button.type = 'button';
    const appearance = {
      'ExploreOccurrence.VisualizerOperator': ['exploreFromHere', '↗'],
      'CloseOccurrence.VisualizerOperator': ['closeOccurrence', '×'],
      'ExpandRestoreOccurrence.VisualizerOperator': ['maximizeInspector', '⤢'],
      'InspectOccurrence.VisualizerOperator': ['visualizerInformation', 'v'],
    }[context.presentationOperator];
    this.glyph = appearance?.[1];
    if (appearance) this.button.dataset[appearance[0]] = 'true';
    this.button.textContent = this.glyph || context.title;
    this.button.style.cssText = 'min-width:28px;max-width:100%;min-height:28px;font:inherit;color:inherit;background:transparent;border:0;cursor:pointer;';
    this.button.addEventListener('click', () => {
      if (!this.isConnected || this.button.disabled) return;
      const result = context.presentationActivation(this.button);
      this.status.textContent = result?.reason || '';
    });
    this.status = document.createElement('span'); this.status.setAttribute('role', 'status');
    this.replaceChildren(this.button, this.status); this.ready = Promise.resolve();
  }
}
