import { DiscoveryExplorerOwner } from './discovery-explorer';
import type { VisualizerDiscovery } from '../deps';
import type { VisualizerInspectionTarget, VisualizerElement } from '../contracts/visualizers';
import type { SpaceNavigatorBinding } from './space-navigator-experience';
import { defineCustomElementOnce } from '../visualizers/define-custom-element-once';
import { DahnHolonView } from '../map-adapter/dahn-holon-view';
import { semanticWork } from './semantic-work';
import { withVisualizerComposition } from './visualizer-information-control';
import { NavigationProfile } from './navigation-profile';

interface InspectionFrame {
  target: VisualizerInspectionTarget;
  technicalOpen: boolean;
  discovery?: Promise<VisualizerDiscovery>;
  explorer?: DiscoveryExplorerOwner;
  mounts: Set<VisualizerElement>;
  children: Map<string, InspectionFrame>;
  disposed: boolean;
  restoreFocus?: boolean;
}

/** Space Navigator owns allocation and dismissal; the selected inspector owns its content. */
export class VisualizerInformationRegion {
  readonly element = document.createElement('aside');
  readonly toggle = document.createElement('button');
  private readonly content = document.createElement('div');
  private readonly close = document.createElement('button');
  private target?: VisualizerInspectionTarget;
  private readonly frames: InspectionFrame[] = [];
  private readonly back = document.createElement('button');
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
    this.back.type = this.toggle.type = this.close.type = 'button';
    this.back.textContent = 'Back to previous information'; this.back.hidden = true;
    this.back.addEventListener('click', () => this.pop());
    this.toggle.textContent = 'Visualizer info';
    this.close.textContent = 'Close visualizer info';
    for (const button of [this.toggle, this.close]) button.style.cssText = 'font:inherit;color:var(--dahn-action-text-color);background:var(--dahn-action-surface-background);padding:var(--dahn-action-padding-block) var(--dahn-action-padding-inline);';
    this.toggle.setAttribute('aria-expanded', 'false');
    this.toggle.addEventListener('click', () => { if (this.open) { this.setOpen(false); this.dismiss(); } else if (this.target?.isLive()) this.inspect(this.target); else { this.setOpen(true); this.close.focus(); } });
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
    this.element.append(this.close, this.back, this.content);
    this.content.textContent = 'Select a Visualizer information control to inspect its presentation.';
    this.observer = new MutationObserver(() => {
      if (this.choosing) return;
      const invalid = this.frames.findIndex(frame => !frame.target.isLive());
      if (invalid === 0) { if (this.mobile.matches) this.setOpen(false); this.dismiss(); }
      else if (invalid > 0) { while (this.frames.length > invalid) this.releaseFrame(this.frames.pop()!); this.showFrame(true); }
      for (const root of this.frames) for (const frame of this.ownedFrames(root)) {
        for (const [identity, child] of frame.children) if (!child.target.isLive()) { this.releaseFrame(child); frame.children.delete(identity); }
        for (const element of frame.mounts) if (!element.isConnected) this.releaseElement(frame, element);
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

  /** A main-area invocation owns a new complete inspection resource chain. */
  inspect(target: VisualizerInspectionTarget): void {
    if (!target.isLive()) return;
    this.choiceAbort?.abort(); this.markTarget(false);
    while (this.frames.length) this.releaseFrame(this.frames.pop()!);
    this.frames.push(this.frame(target));
    this.showFrame();
  }

  private frame(target: VisualizerInspectionTarget): InspectionFrame {
    return { target: withVisualizerComposition(target), technicalOpen: false, mounts: new Set(), children: new Map(), disposed: false };
  }

  private releaseFrame(frame: InspectionFrame): void {
    frame.disposed = true; frame.explorer?.dispose();
    for (const child of frame.children.values()) this.releaseFrame(child);
    frame.children.clear();
    for (const element of frame.mounts) this.releaseElement(frame, element);
    frame.mounts.clear();
    if (!frame.explorer) void frame.discovery?.then(result => result.evidence?.dispose()).catch(console.error);
  }

  private *ownedFrames(frame: InspectionFrame): Generator<InspectionFrame> {
    yield frame;
    for (const child of frame.children.values()) yield* this.ownedFrames(child);
  }

  private releaseElement(frame: InspectionFrame, element: VisualizerElement): void {
    try { element.dispose?.(); }
    catch (error) { console.warn('[DAHN] Inspector presentation release failed', error); }
    finally { element.remove(); frame.mounts.delete(element); }
  }

  private push(target: VisualizerInspectionTarget): void {
    if (!target.isLive()) return;
    if (this.frames.length >= 8) { this.back.title = 'The information nesting limit is eight sessions. Return before opening another.'; this.back.focus(); return; }
    this.choiceAbort?.abort(); this.markTarget(false);
    const parent = this.frames.at(-1);
    if (parent) for (const frame of this.ownedFrames(parent)) {
      frame.explorer?.suspend();
      for (const element of frame.mounts) this.releaseElement(frame, element);
    }
    this.frames.push(this.frame(target)); this.showFrame();
  }

  private pop(): void {
    if (this.frames.length < 2) return;
    this.choiceAbort?.abort(); this.markTarget(false);
    this.releaseFrame(this.frames.pop()!); this.showFrame(true);
  }

  private showFrame(returning = false): void {
    const frame = this.frames.at(-1);
    if (!frame) return;
    this.target = frame.target;
    frame.restoreFocus = returning;
    const revision = ++this.revision;
    this.back.hidden = this.frames.length < 2;
    this.setOpen(true);
    this.content.textContent = 'Opening Visualizer information…'; this.content.setAttribute('aria-busy', 'true');
    if (!returning) this.close.focus();
    const { transaction } = this.binding;
    void semanticWork(transaction).realize(() => this.mountInspection(frame.target, this.content, revision)).then(() => {
      if (returning && revision === this.revision) {
        const counterpart = this.content.querySelector<HTMLElement>('[data-discovery-visualizer-information]');
        if (counterpart) counterpart.focus(); else this.close.focus();
      }
    }).catch(error => {
      if (revision !== this.revision || !frame.target.isLive()) return;
      const alert = document.createElement('p'); alert.setAttribute('role', 'alert');
      alert.textContent = `Unable to read Visualizer information: ${error instanceof Error ? error.message : String(error)}`;
      const retry = document.createElement('button'); retry.type = 'button'; retry.textContent = 'Retry';
      retry.addEventListener('click', () => this.showFrame(returning)); this.content.replaceChildren(alert, retry);
    }).finally(() => { if (revision === this.revision) this.content.removeAttribute('aria-busy'); });
  }

  /** Every nested definition is selected through the same experience-owned information slot. */
  private async mountInspection(target: VisualizerInspectionTarget, host: HTMLElement, revision: number, frame = this.frames.at(-1)): Promise<void> {
    target = withVisualizerComposition(target);
    const { transaction, dancer, materialized, theme, canvas } = this.binding;
    if (!frame) return;
    let presentation: VisualizerElement | undefined;
    const current = () => !frame.disposed && revision === this.revision && target.isLive() && host.isConnected && (!presentation || presentation.isConnected);
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
    presentation = element;
    const retainFocus = host.contains(document.activeElement);
    for (const prior of frame.mounts) if (host.contains(prior)) this.releaseElement(frame, prior);
    host.replaceChildren(element); frame.mounts.add(element);
    const mountChild = (child: VisualizerInspectionTarget, childHost: HTMLElement) => {
      if (!current()) return Promise.resolve();
      let childFrame = frame.children.get(child.occurrenceId);
      if (childFrame && (childFrame.target.context !== child.context || !(childFrame.target.selectedVisualizer === child.selectedVisualizer || childFrame.target.selectedVisualizer.equals(child.selectedVisualizer)))) {
        this.releaseFrame(childFrame); childFrame = undefined;
      }
      if (!childFrame) { childFrame = this.frame(child); frame.children.set(child.occurrenceId, childFrame); }
      return semanticWork(transaction).realize(() => this.mountInspection(child, childHost, revision, childFrame));
    };
    const discover = async () => {
      if (!current()) throw new Error('The inspected occurrence is no longer live.');
      frame.discovery ??= target.choices!.discover().catch(error => { frame.discovery = undefined; throw error; });
      const result = await frame.discovery;
      if (!current()) throw new Error('This information session was superseded.');
      return result;
    };
    const choose = async (candidate: VisualizerInspectionTarget['selectedVisualizer'], signal: AbortSignal) => {
        if (!current() || !this.open || signal.aborted) throw new Error('This information session was closed.');
        this.choiceAbort?.abort();
        const cancellation = this.choiceAbort = new AbortController();
        const abort = () => cancellation.abort();
        signal.addEventListener('abort', abort, { once: true });
        this.choosing = true;
        try {
          const replacement = await target.choices!.choose!(candidate, () => current() && this.open && !signal.aborted, cancellation.signal);
          if (revision !== this.revision || !this.open || !replacement.isLive()) return;
          const outer = frame === this.frames.at(-1);
          if (outer) this.markTarget(false);
          frame.explorer?.dispose(); frame.explorer = undefined;
          for (const child of frame.children.values()) this.releaseFrame(child);
          frame.children.clear();
          if (!frame.explorer) void frame.discovery?.then(result => result.evidence?.dispose()).catch(console.error);
          frame.discovery = undefined;
          frame.target = replacement;
          if (outer) { this.target = replacement; this.markTarget(true); }
          // Refresh the selected identity without another invocation or stealing focus.
          const next = outer ? ++this.revision : this.revision;
          const profile = NavigationProfile.start();
          let outcome = 'visualizer information refresh failed';
          try {
            await semanticWork(transaction).realize(async () => {
              profile?.begin('refresh visualizer information');
              await this.mountInspection(replacement, host, next, frame);
            });
            outcome = 'visualizer information refreshed';
          }
          catch (error) {
            if (next !== this.revision || !replacement.isLive()) return;
            const status = document.createElement('p'); status.setAttribute('role', 'status');
            status.textContent = `Visualizer changed. Unable to refresh its information: ${error instanceof Error ? error.message : String(error)}`;
            const retry = document.createElement('button'); retry.type = 'button'; retry.textContent = 'Retry information';
            retry.addEventListener('click', () => this.inspect(replacement)); host.replaceChildren(status, retry);
          }
          finally { profile?.finish(outcome); }
        } finally {
          signal.removeEventListener('abort', abort);
          if (this.choiceAbort === cancellation) { this.choiceAbort = undefined; this.choosing = false; }
          if (this.target && !this.target.isLive()) this.dismiss();
        }
    };

    element.setContext({ target: { reference: subject }, holon: new DahnHolonView(subject), visualizerInspection: target,
      onInspectVisualizer: child => { if (current()) this.push(child); },
      technicalDetailsOpen: frame.technicalOpen,
      onTechnicalDetailsChanged: open => { if (current()) frame.technicalOpen = open; },
      refreshDiscoveryExplorer: target.choices ? async () => {
        const result = await target.choices!.discover();
        if (!current()) { await result.evidence?.dispose(); return; }
        try {
          if (frame.explorer) await frame.explorer.refresh(result);
          else await (await frame.discovery)?.evidence?.dispose();
          frame.discovery = Promise.resolve(result);
        } catch (error) {
          frame.explorer?.dispose(); frame.explorer = undefined; frame.discovery = undefined;
          await result.evidence?.dispose(); throw error;
        }
      } : undefined,
      mountDiscoveryExplorer: target.choices ? async childHost => {
        if (!current()) return;
        const result = await discover();
        if (!current() || !childHost.isConnected) return;
        frame.explorer ??= new DiscoveryExplorerOwner(this.binding, target, selection.selected, result, child => this.push(child));
        try { await frame.explorer.mount(childHost, { inspectVisualizerCandidate: async (candidate, previewHost) => {
          if (!current()) return;
          const preview = { ...target, occurrenceId: `${target.occurrenceId}:preview:${await candidate.key()}`, selectedVisualizer: candidate, choices: undefined, composition: () => [], regionLabel: undefined };
          await mountChild(preview, previewHost);
        }, chooseVisualizerCandidate: async (candidate, signal) => { await choose(candidate, signal); } }); }
        catch (error) {
          if (current()) {
            frame.explorer?.dispose(); frame.explorer = undefined; frame.discovery = undefined;
          }
          throw error;
        }
        if (current() && frame.restoreFocus) {
          childHost.querySelector<HTMLElement>('[data-discovery-visualizer-information]')?.focus();
          frame.restoreFocus = false;
        }
      } : undefined,
      discoverVisualizerChoices: target.choices ? discover : undefined,
      inspectVisualizerCandidate: target.choices ? async (candidate, previewHost) => {
        if (!current()) return;
        const preview = { ...target, occurrenceId: `${target.occurrenceId}:preview:${await candidate.key()}`, selectedVisualizer: candidate, choices: undefined,
          composition: () => [], regionLabel: undefined };
        await mountChild(preview, previewHost);
      } : undefined,
      chooseVisualizerCandidate: target.choices?.choose ? choose : undefined,
      mountVisualizerInformation: mountChild,
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
    while (this.frames.length) this.releaseFrame(this.frames.pop()!);
    this.back.hidden = true;
    this.content.removeAttribute('aria-busy');
    this.content.textContent = 'Select a Visualizer information control to inspect its presentation.';
  }

  dispose(): void { this.setOpen(false); this.dismiss(); this.mobile.removeEventListener('change', this.adapt); this.observer.disconnect(); this.element.remove(); }
}
