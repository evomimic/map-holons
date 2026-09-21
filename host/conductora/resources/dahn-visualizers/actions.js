export default class ActionsElement extends HTMLElement {
  connectedCallback() {
    if (!this.layout) return;
    this.observer?.disconnect();
    this.layout.connect();
    this.observer = new ResizeObserver(() => this.scheduleLayout());
    this.layout.elements.forEach(element => this.observer.observe(element));
    this.scheduleLayout();
  }
  disconnectedCallback() {
    this.observer?.disconnect();
    if (this.frame != null) cancelAnimationFrame(this.frame);
    this.frame = null;
    this.layout?.dispose();
  }
  scheduleLayout() {
    if (this.frame != null) return;
    this.frame = requestAnimationFrame(() => { this.frame = null; if (this.isConnected) this.layout.fit(); });
  }
  setContext(context) {
    this.dataset.dahnNodeActions = 'true';
    this.disconnectedCallback();
    const controls = context.actions.map(action => {
      const button = document.createElement('button');
      button.type = 'button';
      button.textContent = action.label;
      button.disabled = true;
      button.title = 'Action activation is not available yet';
      button.dataset.actionId = action.id;
      button.style.font = 'inherit';
      button.style.border = 'var(--dahn-slot-border-width) solid var(--dahn-slot-border-color)';
      button.style.opacity = '1';
      button.style.color = 'var(--dahn-action-text-color)';
      button.style.padding = 'var(--dahn-action-padding-block) var(--dahn-action-padding-inline)';
      button.style.borderRadius = 'var(--dahn-action-corner-radius)';
      button.style.background = 'var(--dahn-action-surface-background)';
      return button;
    });
    this.layout = horizontalOverflow(this, controls, 'More actions');
    if (this.isConnected) this.connectedCallback();
    if (controls.length === 0) this.textContent = 'No actions';
  }
}

let nextOverflowId = 0;

// A disclosure is presentation-only; the contained semantic controls stay disabled.
function horizontalOverflow(host, controls, label) {
  Object.assign(host.style, { display: 'block', minWidth: '0', maxWidth: '100%', position: 'relative' });
  const row = document.createElement('div');
  row.dataset.overflowRow = 'true';
  Object.assign(row.style, { display: 'flex', flexWrap: 'nowrap', alignItems: 'center', gap: 'var(--dahn-control-gap)', minWidth: '0', overflow: 'hidden' });
  const more = document.createElement('button');
  more.type = 'button';
  more.dataset.overflowMore = 'true';
  more.textContent = label;
  more.setAttribute('aria-expanded', 'false');
  Object.assign(more.style, { font: 'inherit', cursor: 'pointer', border: '0', padding: 'var(--dahn-action-padding-block) var(--dahn-action-padding-inline)', color: 'var(--dahn-action-text-color)', background: 'var(--dahn-action-surface-background)', borderRadius: 'var(--dahn-action-corner-radius)' });
  Object.assign(more.style, { flex: '0 0 auto', maxWidth: '100%', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' });
  const popup = document.createElement('div');
  popup.dataset.overflowPopup = 'true';
  popup.id = `dahn-${label.toLowerCase().replaceAll(" ", "-")}-${++nextOverflowId}`;
  more.setAttribute('aria-controls', popup.id);
  popup.setAttribute('popover', 'auto');
  popup.setAttribute('role', 'region');
  popup.setAttribute('aria-label', label);
  popup.tabIndex = -1;
  Object.assign(popup.style, {
    position: 'fixed', margin: '0', boxSizing: 'border-box', overflowY: 'auto',
    padding: 'var(--dahn-control-gap)', borderRadius: 'var(--dahn-panel-corner-radius)',
    color: 'var(--dahn-canvas-text-color)', background: 'var(--dahn-canvas-surface-background)',
    border: 'var(--dahn-slot-border-width) var(--dahn-slot-border-style) var(--dahn-slot-border-color)',
  });
  popup.hidden = true;
  let opened = false;
  let hidden = [];
  const close = () => {
    if (opened && popup.hidePopover) popup.hidePopover();
    opened = false; popup.hidden = true; more.setAttribute('aria-expanded', 'false');
  };
  const place = () => {
    const anchor = more.getBoundingClientRect();
    const width = Math.min(Math.max(anchor.width, popup.scrollWidth), window.innerWidth);
    popup.style.maxWidth = `${window.innerWidth}px`;
    popup.style.left = `${Math.max(0, Math.min(anchor.left, window.innerWidth - width))}px`;
    const below = window.innerHeight - anchor.bottom;
    const upward = below < Math.min(popup.scrollHeight, anchor.top);
    popup.style.top = upward ? 'auto' : `${anchor.bottom}px`;
    popup.style.bottom = upward ? `${window.innerHeight - anchor.top}px` : 'auto';
    popup.style.maxHeight = `${Math.max(0, upward ? anchor.top : below)}px`;
  };
  more.addEventListener('click', () => {
    if (opened) { close(); return; }
    popup.replaceChildren(...hidden.map(control => {
      const copy = control.cloneNode(true);
      copy.style.marginBottom = 'var(--dahn-control-gap)';
      copy.removeAttribute('aria-hidden'); copy.inert = false;
      Object.assign(copy.style, { position: 'static', visibility: 'visible', display: 'block', width: '100%', maxWidth: '100%', whiteSpace: 'normal', overflowWrap: 'anywhere' });
      return copy;
    }));
    popup.hidden = false;
    if (popup.showPopover) popup.showPopover();
    opened = true; more.setAttribute('aria-expanded', 'true'); place(); popup.focus();
  });
  const escape = event => { if (opened && event.key === 'Escape') { close(); more.focus(); } };
  const outside = event => { if (opened && !popup.contains(event.target) && !more.contains(event.target)) close(); };
  popup.addEventListener('toggle', event => { if (event.newState === 'closed') { opened = false; popup.hidden = true; more.setAttribute('aria-expanded', 'false'); } });
  for (const control of controls) Object.assign(control.style, { flex: '0 0 auto', width: 'max-content', whiteSpace: 'nowrap' });
  row.append(...controls, more); host.replaceChildren(row, popup);
  const show = (element, visible) => {
    Object.assign(element.style, { position: visible ? 'static' : 'absolute', visibility: visible ? 'visible' : 'hidden' });
    element.inert = !visible; element.setAttribute('aria-hidden', String(!visible));
  };
  const fit = () => {
    const width = row.clientWidth;
    const gap = parseFloat(getComputedStyle(row).columnGap) || 0;
    const widths = controls.map(control => control.getBoundingClientRect().width);
    const total = widths.reduce((a, b) => a + b, 0) + Math.max(0, widths.length - 1) * gap;
    const overflow = total > width;
    const budget = overflow ? Math.max(0, width - more.getBoundingClientRect().width - gap) : width;
    let count = 0, used = 0;
    for (const value of widths) {
      const next = used + (count ? gap : 0) + value;
      if (next > budget) break;
      used = next; count++;
    }
    hidden = controls.slice(count);
    controls.forEach((control, index) => show(control, index < count));
    if (!overflow && document.activeElement === more) { host.tabIndex = -1; host.focus(); }
    show(more, overflow);
    close();
  };
  return { fit, elements: [host, row, more, ...controls], connect() { document.addEventListener('keydown', escape); document.addEventListener('pointerdown', outside); }, dispose() { close(); document.removeEventListener('keydown', escape); document.removeEventListener('pointerdown', outside); } };
}
