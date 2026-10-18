import { actionIcon } from './action-icons.js';
export default class ActionsElement extends HTMLElement {
  get visualizerOperations() {
    return { 'PresentActions.VisualizerOperator': input => {
      if (!Array.isArray(input?.actions)) throw new Error('PresentActions requires supplied actions');
      this.setActions(input.actions, input.overflow !== false);
      return { status: 'applied' };
    } };
  }
  getVisualizerComposition() { return []; }
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
  setActions(actions, allowOverflow = true) {
    const validate = (items, seen = new Set()) => {
      for (const action of items) {
        if (!action.id || !action.label || seen.has(action.id)) throw new Error('Invalid or duplicate Action identity');
        seen.add(action.id);
        if (action.kind === 'group') validate(action.children ?? [], seen);
        else if (action.kind !== 'action' || typeof action.activate !== 'function' || typeof action.available !== 'boolean'
          || !action.available && !action.reason) throw new Error(`Invalid bound Action: ${action.id}`);
      }
    };
    validate(actions);
    this.dataset.dahnNodeActions = 'true';
    const previous = this.controls ?? new Map();
    const controls = new Map();
    const compose = items => items.map(action => {
      if (action.kind === 'group') {
        const group = previous.get(action.id) ?? document.createElement('div');
        group.setAttribute('role', 'group'); group.setAttribute('aria-label', action.label);
        group.style.cssText = 'display:flex;align-items:center;gap:var(--dahn-control-gap);';
        controls.set(action.id, group);
        const children = compose(action.children ?? []);
        if (children.length !== group.children.length || children.some((child, index) => group.children[index] !== child)) group.replaceChildren(...children);
        return group;
      }
      const button = previous.get(action.id) ?? document.createElement('button');
      button.type = 'button'; button.dataset.actionId = action.id;
      const icon = actionIcon(action.id, action.label === 'Restore');
      if (icon) button.replaceChildren(icon);
      else button.textContent = action.icon || action.label;
      button.disabled = !action.available;
      button.title = action.available ? action.label : `${action.label}: ${action.reason}`;
      button.setAttribute('aria-label', button.title);
      button.style.cssText = 'box-sizing:border-box;max-height:100%;font:inherit;min-width:28px;min-height:28px;max-width:100%;color:var(--dahn-action-text-color);background:var(--dahn-action-surface-background);padding:var(--dahn-action-padding-block) var(--dahn-action-padding-inline);border:0;border-radius:var(--dahn-action-corner-radius);cursor:pointer;';
      button.onclick = () => {
        if (!button.isConnected || button.disabled) return;
        const report = result => {
          this.status.textContent = result?.reason || ''; this.notice.hidden = !this.status.textContent;
          if (!this.notice.hidden) { this.lastInvoker = button; this.dismiss.focus(); }
        };
        try { const result = action.activate(button);
          if (result?.then) result.then(report, error => report({ reason: error.message })); else report(result); }
        catch (error) { report({ reason: error.message }); }
      };
      controls.set(action.id, button); return button;
    });
    const sameShape = JSON.stringify(actions.map(shape)) === this.shape && allowOverflow === this.allowOverflow;
    function shape(action) { return [action.id, action.kind, action.children?.map(shape)]; }
    const components = compose(actions);
    this.controls = controls;
    if (!sameShape) {
      this.disconnectedCallback();
      if (!this.surface) {
        Object.assign(this.style, { display: 'block', position: 'relative', height: '100%', minHeight: '0', minWidth: '0', maxWidth: '100%', boxSizing: 'border-box', overflow: 'hidden' });
        this.surface = document.createElement('div'); this.append(this.surface);
        this.notice = document.createElement('div'); this.notice.hidden = true;
        this.notice.style.cssText = 'position:absolute;inset:0;z-index:3;overflow:auto;background:var(--dahn-action-surface-background);color:var(--dahn-action-text-color);';
        this.status = document.createElement('span'); this.status.setAttribute('role', 'status');
        this.dismiss = document.createElement('button'); this.dismiss.type = 'button'; this.dismiss.textContent = 'Dismiss';
        this.dismiss.addEventListener('click', () => { this.notice.hidden = true; this.lastInvoker?.focus(); });
        this.notice.append(this.status, this.dismiss); this.append(this.notice);
      }
      this.layout = horizontalOverflow(this.surface, components, 'More actions', allowOverflow);
      this.allowOverflow = allowOverflow;
      this.shape = JSON.stringify(actions.map(shape));
      if (this.isConnected) this.connectedCallback();
    }
    this.scheduleLayout();
  }
}

let nextOverflowId = 0;

// Overflow moves live Action components, preserving their interaction bindings.
function horizontalOverflow(host, controls, label, allowOverflow = true) {
  Object.assign(host.style, { display: 'block', minWidth: '0', maxWidth: '100%', height: '100%', maxHeight: '100%', boxSizing: 'border-box', overflow: 'hidden', position: 'relative' });
  const row = document.createElement('div');
  row.dataset.overflowRow = 'true';
  Object.assign(row.style, { display: 'flex', flexWrap: 'nowrap', alignItems: 'center', gap: 'var(--dahn-control-gap)', minWidth: '0', height: '100%', overflow: 'hidden' });
  const more = document.createElement('button');
  more.type = 'button';
  more.dataset.overflowMore = 'true';
  more.textContent = label;
  if (!allowOverflow) { more.hidden = true; more.style.display = 'none'; }
  more.setAttribute('aria-expanded', 'false');
  Object.assign(more.style, { boxSizing: 'border-box', maxHeight: '100%', font: 'inherit', cursor: 'pointer', border: '0', padding: 'var(--dahn-action-padding-block) var(--dahn-action-padding-inline)', color: 'var(--dahn-action-text-color)', background: 'var(--dahn-action-surface-background)', borderRadius: 'var(--dahn-action-corner-radius)' });
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
    if (!allowOverflow) { hidden = []; controls.forEach(control => show(control, true)); show(more, false); return; }
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
