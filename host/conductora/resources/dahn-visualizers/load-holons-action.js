export default class LoadHolonsAction extends HTMLElement {
  setContext(context) {
    const button = document.createElement('button');
    button.type = 'button';
    button.textContent = context.actionActivation.binding.label;
    button.disabled = !context.actionInteractions;
    button.title = button.disabled ? 'Native source interaction is unavailable' : 'Upload from computer';
    Object.assign(button.style, { font: 'inherit', minHeight: '2.25rem', width: '100%', color: 'var(--dahn-action-text-color)', background: 'var(--dahn-action-surface-background)', padding: 'var(--dahn-action-padding-block) var(--dahn-action-padding-inline)', borderRadius: 'var(--dahn-action-corner-radius)', border: 'var(--dahn-slot-border-width) solid var(--dahn-slot-border-color)' });
    button.addEventListener('click', () => context.actionActivation.activate(binding => context.actionInteractions.openLoadHolons(binding)));
    this.replaceChildren(button);
  }
}
