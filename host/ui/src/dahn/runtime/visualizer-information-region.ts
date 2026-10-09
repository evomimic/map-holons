import type { VisualizerInspectionTarget, VisualizerElement } from '../contracts/visualizers';
import type { SpaceNavigatorBinding } from './space-navigator-experience';
import { defineCustomElementOnce } from '../visualizers/define-custom-element-once';
import { DahnHolonView } from '../map-adapter/dahn-holon-view';
import { semanticWork } from './semantic-work';
import { withVisualizerComposition } from './visualizer-information-control';

/** Space Navigator owns allocation and dismissal; the selected inspector owns its content. */
export class VisualizerInformationRegion {
  readonly element = document.createElement('aside');
  readonly toggle = document.createElement('button');
  private readonly content = document.createElement('div');
  private readonly close = document.createElement('button');
  private target?: VisualizerInspectionTarget;
  private revision = 0;
  private open = false;
  private choosing = false;
  private choiceAbort?: AbortController;
  private readonly observer: MutationObserver;
  private readonly mobile = window.matchMedia('(max-width: 700px)');
  private readonly adapt = () => {
    const navigation = this.host.querySelector<HTMLElement>('[data-exploration-navigation]');
    if (navigation) navigation.inert = this.open && this.mobile.matches;
    this.element.setAttribute('role', this.mobile.matches ? 'dialog' : 'complementary');
    if (this.mobile.matches) this.element.setAttribute('aria-modal', String(this.open));
    else this.element.removeAttribute('aria-modal');
  };

  constructor(private readonly binding: SpaceNavigatorBinding, private readonly host: HTMLElement,
    private readonly explore?: (reference: VisualizerInspectionTarget['selectedVisualizer']) => void) {
    this.element.dataset['visualizerInformationRegion'] = 'true';
    this.element.setAttribute('aria-label', 'Visualizer information');
    this.element.tabIndex = -1;
    this.element.style.cssText = 'display:none;box-sizing:border-box;flex:0 0 360px;width:360px;min-width:0;min-height:0;overflow:auto;padding:var(--dahn-slot-padding);background:var(--dahn-panel-surface-background);border-left:var(--dahn-slot-border-width) solid var(--dahn-slot-border-color);';
    this.toggle.type = this.close.type = 'button';
    this.toggle.textContent = 'Visualizer info';
    this.close.textContent = 'Close visualizer info';
    for (const button of [this.toggle, this.close]) button.style.cssText = 'font:inherit;color:var(--dahn-action-text-color);background:var(--dahn-action-surface-background);padding:var(--dahn-action-padding-block) var(--dahn-action-padding-inline);';
    this.toggle.setAttribute('aria-expanded', 'false');
    this.toggle.addEventListener('click', () => { if (this.open) { this.setOpen(false); this.choiceAbort?.abort(); ++this.revision; } else if (this.target?.isLive()) this.inspect(this.target); else { this.setOpen(true); this.close.focus(); } });
    this.close.addEventListener('click', () => { this.setOpen(false); this.dismiss(); });
    this.element.addEventListener('keydown', event => {
      if (event.key === 'Escape') { event.preventDefault(); this.setOpen(false); this.dismiss(); }
      if (event.key === 'Tab' && this.mobile.matches) {
        const controls = [...this.element.querySelectorAll<HTMLElement>('button, summary, a[href], [tabindex="0"]')].filter(control => {
          for (let parent: HTMLElement | null = control; parent && parent !== this.element; parent = parent.parentElement) {
            if (parent.hidden || parent.inert || getComputedStyle(parent).display === 'none') return false;
            if (parent instanceof HTMLDetailsElement && !parent.open && !parent.querySelector(':scope > summary')?.contains(control)) return false;
          }
          return true;
        });
        const first = controls[0], last = controls.at(-1);
        if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
        else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
      }
    });
    this.mobile.addEventListener('change', this.adapt);
    this.element.append(this.close, this.content);
    this.content.textContent = 'Select a Visualizer information control to inspect its presentation.';
    this.observer = new MutationObserver(() => {
      if (this.target && !this.target.isLive() && !this.choosing) {
        if (this.mobile.matches) this.setOpen(false);
        this.dismiss();
      }
    });
    this.observer.observe(host, { childList: true, subtree: true });
    const style = document.createElement('style');
    style.textContent = '[data-visualizer-inspected="true"] { color:var(--dahn-action-surface-background)!important;background:var(--dahn-action-text-color)!important; } @media (max-width: 700px) { [data-visualizer-information-region] { position:absolute;inset:0;z-index:20;width:auto!important;max-width:100%; } }';
    this.element.append(style);
  }

  private setOpen(open: boolean): void {
    this.open = open;
    this.element.style.display = open ? 'block' : 'none';
    this.toggle.setAttribute('aria-expanded', String(open));
    this.adapt();
    this.markTarget(open);
    if (!open) this.returnFocus();
  }

  private markTarget(selected: boolean): void {
    const invoker = this.target?.invoker;
    if (!invoker) return;
    invoker.setAttribute('aria-pressed', String(selected));
    if (selected) invoker.dataset['visualizerInspected'] = 'true';
    else delete invoker.dataset['visualizerInspected'];
  }

  private returnFocus(): void {
    const invoker = this.target?.invoker;
    if (invoker?.isConnected && !invoker.closest('[inert]')) invoker.focus();
    else this.toggle.focus();
  }

  /** New invocation replaces only this information session, never navigation or selection. */
  inspect(target: VisualizerInspectionTarget): void {
    if (!target.isLive()) return;
    this.choiceAbort?.abort();
    target = withVisualizerComposition(target);
    this.markTarget(false);
    this.target = target;
    const revision = ++this.revision;
    this.setOpen(true);
    this.content.textContent = 'Opening Visualizer information…';
    this.content.setAttribute('aria-busy', 'true');
    this.close.focus();
    const { transaction } = this.binding;
    void semanticWork(transaction).realize(() => this.mountInspection(target, this.content, revision)).catch(error => {
      if (revision !== this.revision || !target.isLive()) return;
      const alert = document.createElement('p');
      alert.setAttribute('role', 'alert');
      alert.textContent = `Unable to read Visualizer information: ${error instanceof Error ? error.message : String(error)}`;
      const retry = document.createElement('button'); retry.type = 'button'; retry.textContent = 'Retry';
      retry.addEventListener('click', () => this.inspect(target));
      this.content.replaceChildren(alert, retry);
    }).finally(() => { if (revision === this.revision) this.content.removeAttribute('aria-busy'); });
  }

  /** Every nested definition is selected through the same experience-owned information slot. */
  private async mountInspection(target: VisualizerInspectionTarget, host: HTMLElement, revision: number): Promise<void> {
    target = withVisualizerComposition(target);
    const { transaction, dancer, materialized, theme, canvas } = this.binding;
    const current = () => revision === this.revision && target.isLive() && host.isConnected;
    if (!current()) return;
    const slots = [...await dancer.relatedHolons('HasExperienceVisualizerSlot')];
    const matches = [];
    for (const slot of slots) if (await slot.key() === 'SpaceNavigator.VisualizerInformationSlot') matches.push(slot);
    if (matches.length !== 1) throw new Error('Space Navigator requires one Visualizer information slot.');
    const subject = transaction.owns(target.selectedVisualizer) ? target.selectedVisualizer : transaction.bindSavedReference(target.selectedVisualizer);
    const selection = await transaction.selectVisualizer({ subject, requestedKind: 'node', slot: matches[0], owner: { dancer }, theme: theme.reference });
    const implementation = await materialized.realize(selection.selected);
    if (typeof implementation !== 'function' || !(implementation.prototype instanceof HTMLElement)) throw new Error('Selected Visualizer inspector is not an HTMLElement constructor.');
    if (!current()) return;
    const element = document.createElement(defineCustomElementOnce('map-visualizer-inspector', implementation as CustomElementConstructor)) as VisualizerElement;
    const retainFocus = host.contains(document.activeElement);
    host.replaceChildren(element);
    element.setContext({ target: { reference: subject }, holon: new DahnHolonView(subject), visualizerInspection: target,
      onInspectVisualizer: child => this.inspect(child),
      discoverVisualizerChoices: target.choices ? async () => {
        if (!current()) throw new Error('The inspected occurrence is no longer live.');
        const result = await target.choices!.discover();
        if (!current()) throw new Error('This information session was superseded.');
        return result;
      } : undefined,
      inspectVisualizerCandidate: target.choices ? async (candidate, previewHost) => {
        if (!current()) return;
        const preview = { ...target, selectedVisualizer: candidate, choices: undefined,
          composition: () => [], regionLabel: undefined };
        await semanticWork(transaction).realize(() => this.mountInspection(preview, previewHost, revision));
      } : undefined,
      chooseVisualizerCandidate: target.choices ? async (candidate, signal) => {
        if (!current() || !this.open || signal.aborted) throw new Error('This information session was closed.');
        this.choiceAbort?.abort();
        const cancellation = this.choiceAbort = new AbortController();
        const abort = () => cancellation.abort();
        signal.addEventListener('abort', abort, { once: true });
        this.choosing = true;
        try {
          const replacement = await target.choices!.choose(candidate, () => current() && this.open && !signal.aborted, cancellation.signal);
          if (revision !== this.revision || !this.open || !replacement.isLive()) return;
          this.markTarget(false);
          this.target = replacement;
          this.markTarget(true);
          // Refresh the selected identity without another invocation or stealing focus.
          const next = ++this.revision;
          try { await semanticWork(transaction).realize(() => this.mountInspection(replacement, host, next)); }
          catch (error) {
            if (next !== this.revision || !replacement.isLive()) return;
            const status = document.createElement('p'); status.setAttribute('role', 'status');
            status.textContent = `Visualizer changed. Unable to refresh its information: ${error instanceof Error ? error.message : String(error)}`;
            const retry = document.createElement('button'); retry.type = 'button'; retry.textContent = 'Retry information';
            retry.addEventListener('click', () => this.inspect(replacement)); host.replaceChildren(status, retry);
          }
        } finally {
          signal.removeEventListener('abort', abort);
          if (this.choiceAbort === cancellation) { this.choiceAbort = undefined; this.choosing = false; }
          if (this.target && !this.target.isLive()) this.dismiss();
        }
      } : undefined,
      mountVisualizerInformation: (child, childHost) => current()
        ? semanticWork(transaction).realize(() => this.mountInspection(child, childHost, revision))
        : Promise.resolve(),
      onExploreVisualizer: this.explore ? () => {
        if (current() && !semanticWork(transaction).paused) this.explore!(subject);
      } : undefined,
      experience: { dancer, holonSpace: this.binding.holonSpace }, actions: [], theme, canvas });
    await element.ready;
    if (retainFocus && current() && document.activeElement === document.body) {
      const heading = element.querySelector<HTMLElement>('h2') ?? element;
      heading.tabIndex = -1; heading.focus();
    }
  }

  dismiss(): void {
    this.choiceAbort?.abort();
    ++this.revision;
    this.markTarget(false);
    this.returnFocus();
    this.target = undefined;
    this.content.removeAttribute('aria-busy');
    this.content.textContent = 'Select a Visualizer information control to inspect its presentation.';
  }

  dispose(): void { this.setOpen(false); this.dismiss(); this.mobile.removeEventListener('change', this.adapt); this.observer.disconnect(); this.element.remove(); }
}
