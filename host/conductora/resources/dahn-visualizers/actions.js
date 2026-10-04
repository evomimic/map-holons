export default class ActionsElement extends HTMLElement {
  static compositionSlots = { action: "GenericActions.ActionSlot" };
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
    const compose = actions => actions.map(action => {
      if (action.kind !== 'group') {
        const child = context.childVisualizers?.get(action.id);
        if (!child) throw new Error(`Missing selected Action Visualizer: ${action.id}`);
        child.dataset.actionId = action.id;
        return child;
      }
      const group = document.createElement('div');
      group.setAttribute('role', 'group');
      group.setAttribute('aria-label', action.label);
      Object.assign(group.style, { display: 'flex', alignItems: 'stretch', gap: 'var(--dahn-control-gap)', borderInlineStart: '1px solid var(--dahn-slot-border-color)', paddingInlineStart: 'var(--dahn-control-gap)' });
      group.append(...compose(action.children ?? []));
      return group;
    });
    const controls = compose(context.actions);
    this.layout = horizontalOverflow(this, controls, 'More actions', [...(context.childVisualizers?.values() ?? [])]);
    if (this.isConnected) this.connectedCallback();
    if (controls.length === 0) this.textContent = 'No actions';
  }
}

let nextOverflowId = 0;

// Overflow moves the live selected controls, preserving their interaction bindings.
function horizontalOverflow(host, controls, label, slots) {
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
    for (const control of controls) row.insertBefore(control, more);
    controls.forEach(control => show(control, !hidden.includes(control)));
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
    popup.replaceChildren(...hidden);
    hidden.forEach(control => show(control, true));
    popup.hidden = false;
    if (popup.showPopover) popup.showPopover();
    opened = true; more.setAttribute('aria-expanded', 'true'); place(); popup.focus();
  });
  popup.addEventListener('click', event => { if (event.target.closest('button') && !event.target.closest('button').disabled) close(); });
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
    if (opened) { place(); return; }
    close();
    slots.forEach(slot => { slot.style.width = 'max-content'; slot.style.minHeight = ''; });
    const slotWidth = Math.max(0, ...slots.map(slot => slot.offsetWidth));
    const slotHeight = Math.max(0, ...slots.map(slot => slot.offsetHeight));
    slots.forEach(slot => { slot.style.width = `${slotWidth}px`; slot.style.minHeight = `${slotHeight}px`; });
    const width = row.clientWidth;
    const gap = parseFloat(getComputedStyle(row).columnGap) || 0;
    const widths = controls.map(control => control.offsetWidth);
    const total = widths.reduce((a, b) => a + b, 0) + Math.max(0, widths.length - 1) * gap;
    const overflow = total > width;
    const budget = overflow ? Math.max(0, width - more.offsetWidth - gap) : width;
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
