import { composedSpatialExtents } from './composed-spatial-extents';
import { defineCustomElementOnce } from '../visualizers/define-custom-element-once';
import type { HolonReference } from '../deps';
import type { VisualizerElement } from '../contracts/visualizers';
import type { SurfaceViewRequest } from '../contracts/canvas';
import type { OccurrenceAttentionRequest, PresentationRequestResult } from '../contracts/presentation';

export interface ExplorationPresentation {
  readonly element: VisualizerElement;
  readonly title: string;
  dispose(): void;
}

export type RealizeExploration = (anchor: HolonReference, signal: AbortSignal) => Promise<ExplorationPresentation>;

interface ExplorationTab {
  id: string;
  controller: AbortController;
  button: HTMLButtonElement;
  controls: HTMLElement;
  panel: HTMLElement;
  presentation?: ExplorationPresentation;
}

let nextTab = 0;

/** Dancer-owned presentation lifecycle. The factory retains semantic execution ownership. */
export class ExplorationTabs extends HTMLElement {
  private readonly tabs = new Map<string, ExplorationTab>();
  readonly actions = document.createElement('div');
  private readonly tablist = document.createElement('div');
  private readonly panels = document.createElement('div');
  private readonly feedback = document.createElement('div');
  private active?: ExplorationTab;
  private activationRevision = 0;
  private disposed = false;

  constructor(private readonly realize: RealizeExploration) {
    super();
    this.dataset['explorationTabs'] = 'true';
    Object.assign(this.style, { display: 'flex', flexDirection: 'column', flex: '1 1 auto', minHeight: '0', minWidth: '0', gap: 'var(--dahn-canvas-gap)' });
    this.tablist.setAttribute('role', 'tablist');
    this.tablist.setAttribute('aria-label', 'Space Navigator explorations');
    Object.assign(this.tablist.style, { display: 'flex', flexWrap: 'wrap', gap: 'var(--dahn-control-gap)' });
    Object.assign(this.panels.style, { position: 'relative', display: 'flex', flex: '1 1 auto', minHeight: '0', minWidth: '0' });
    this.tablist.addEventListener('keydown', event => {
      const ready = [...this.tabs.values()].filter(tab => tab.presentation);
      const index = ready.findIndex(tab => tab.button === event.target);
      if (index < 0 || !['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
      event.preventDefault();
      const target = event.key === 'Home' ? 0 : event.key === 'End' ? ready.length - 1
        : (index + (event.key === 'ArrowRight' ? 1 : -1) + ready.length) % ready.length;
      this.activate(ready[target], true);
      ready[target].button.focus();
    });
    const toolbar = document.createElement('div');
    Object.assign(toolbar.style, { display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 'var(--dahn-control-gap)', flexWrap: 'wrap' });
    toolbar.append(this.tablist, this.actions);
    this.append(toolbar, this.feedback, this.panels);
  }

  getSpatialExtents() {
    const child = this.active?.presentation?.element;
    const report = child?.getSpatialExtents?.();
    return child && report ? composedSpatialExtents(this, child, report) : undefined;
  }

  /** A pending tab acknowledges intent immediately; failures resolve locally. */
  async open(anchor: HolonReference): Promise<void> {
    if (this.disposed) return;
    const revision = this.activationRevision;
    const id = `exploration-${++nextTab}`;
    const controls = document.createElement('div');
    const button = this.button('Opening exploration…');
    button.id = `${id}-tab`;
    button.setAttribute('role', 'tab');
    button.setAttribute('aria-controls', id);
    button.setAttribute('aria-selected', 'false');
    button.setAttribute('aria-busy', 'true');
    button.disabled = true;
    const close = this.button('×');
    Object.assign(close.style, { padding: '0', width: '28px', minHeight: '32px', flex: '0 0 28px', background: 'transparent', border: '0' });
    Object.assign(controls.style, { display: 'flex', alignItems: 'stretch', overflow: 'hidden', borderRadius: 'var(--dahn-action-corner-radius)', border: 'var(--dahn-slot-border-width) solid var(--dahn-slot-border-color)', background: 'var(--dahn-action-surface-background)' });
    Object.assign(button.style, { border: '0', borderRadius: '0', background: 'transparent', maxWidth: '24ch', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' });
    close.setAttribute('aria-label', 'Cancel pending exploration');
    const panel = document.createElement('section');
    panel.id = id;
    panel.setAttribute('role', 'tabpanel');
    panel.setAttribute('aria-labelledby', button.id);
    Object.assign(panel.style, { display: 'flex', flexDirection: 'column', flex: '1 1 auto', minHeight: '0', minWidth: '0' });
    const tab: ExplorationTab = { id, controller: new AbortController(), button, controls, panel };
    button.addEventListener('click', () => this.activate(tab, true));
    close.addEventListener('click', () => this.close(tab));
    controls.append(button, close);
    this.tabs.set(id, tab);
    this.tablist.append(controls);
    this.panels.append(panel);
    this.show(tab, false);
    try {
      const presentation = await this.realize(anchor, tab.controller.signal);
      if (this.disposed || !this.tabs.has(id)) { presentation.dispose(); return; }
      tab.presentation = presentation;
      panel.append(presentation.element);
      button.textContent = presentation.title;
      button.disabled = false;
      button.removeAttribute('aria-busy');
      close.textContent = '×';
      close.setAttribute('aria-label', `Close exploration ${presentation.title}`);
      if (revision === this.activationRevision || !this.active) this.activate(tab, false);
      else button.dataset['ready'] = 'true';
    } catch (error) {
      if (!this.tabs.has(id) || this.disposed) return;
      this.close(tab);
      const alert = document.createElement('div');
      alert.setAttribute('role', 'alert');
      const message = document.createElement('span');
      const detail = error instanceof Error && 'payload' in error && typeof error.payload === 'string'
        ? `${error.message}: ${error.payload}` : error instanceof Error ? error.message : String(error);
      message.textContent = `Unable to open exploration: ${detail}`;
      Object.assign(alert.style, { display: 'flex', alignItems: 'center', flexWrap: 'wrap', gap: 'var(--dahn-control-gap)' });
      const retry = this.button('Retry');
      retry.addEventListener('click', () => { alert.remove(); void this.open(anchor); });
      const dismiss = this.button('Dismiss');
      dismiss.addEventListener('click', () => alert.remove());
      alert.append(message, retry, dismiss);
      this.feedback.append(alert);
    }
  }

  private button(label: string): HTMLButtonElement {
    const button = document.createElement('button');
    button.type = 'button';
    button.textContent = label;
    Object.assign(button.style, { font: 'inherit', color: 'var(--dahn-action-text-color)', background: 'var(--dahn-action-surface-background)', padding: 'var(--dahn-action-padding-block) var(--dahn-action-padding-inline)', borderRadius: 'var(--dahn-action-corner-radius)' });
    return button;
  }

  private show(tab: ExplorationTab, visible: boolean): void {
    // Keep inactive visualizers connected and allocated: disconnect disposes their
    // navigation, and zero-sized viewports can reset layout and scroll position.
    // Opacity hides the entire subtree even when a child overrides visibility.
    Object.assign(tab.panel.style, visible
      ? { position: 'relative', opacity: '1', visibility: 'visible', inset: '', pointerEvents: '' }
      : { position: 'absolute', opacity: '0', visibility: 'hidden', inset: '0', pointerEvents: 'none' });
    tab.panel.inert = !visible;
    tab.panel.setAttribute('aria-hidden', String(!visible));
    tab.button.setAttribute('aria-selected', String(visible));
    tab.button.tabIndex = visible ? 0 : -1;
    tab.button.style.fontWeight = visible ? 'bold' : 'normal';
    tab.controls.style.borderColor = visible ? 'var(--dahn-focus-ring-color)' : 'var(--dahn-slot-border-color)';
    tab.controls.style.boxShadow = visible ? 'inset 0 -2px var(--dahn-focus-ring-color)' : '';
  }

  private activate(tab: ExplorationTab, user: boolean): void {
    if (this.disposed || !this.tabs.has(tab.id) || !tab.presentation) return;
    if (user) ++this.activationRevision;
    const transferFocus = this.active?.panel.contains(document.activeElement);
    if (this.active) this.show(this.active, false);
    this.active = tab;
    this.show(tab, true);
    delete tab.button.dataset['ready'];
    if (transferFocus) tab.button.focus();
  }

  private close(tab: ExplorationTab): void {
    if (!this.tabs.delete(tab.id)) return;
    tab.controller.abort();
    tab.presentation?.dispose();
    tab.controls.remove();
    tab.panel.remove();
    if (this.active === tab) {
      this.active = undefined;
      ++this.activationRevision;
      const next = [...this.tabs.values()].find(item => item.presentation);
      if (next) { this.activate(next, false); next.button.focus(); }
    }
  }

  requestView(request: SurfaceViewRequest): boolean {
    return this.active?.presentation?.element.requestView?.(request) ?? false;
  }

  requestAttention(request: OccurrenceAttentionRequest): PresentationRequestResult {
    return this.active?.presentation?.element.requestAttention?.(request)
      ?? { status: 'refused', reason: 'No active exploration accepts attention.' };
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    for (const tab of [...this.tabs.values()]) this.close(tab);
    this.feedback.replaceChildren();
    this.remove();
  }
}

// This is Dancer composition, not a selected Visualizer implementation.
defineCustomElementOnce('map-exploration-tabs', ExplorationTabs);
