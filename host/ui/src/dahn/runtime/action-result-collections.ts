import { bindCollectionVisualizerInformation } from './realize-collection';
import type { DescribedHolonCollection, HolonReference, MapTransaction } from '../deps';
import type { DahnTheme } from '../contracts/themes';
import type { ActionBinding } from './action-activation';
import type { MaterializedVisualizerRuntime } from './materialized-visualizer-runtime';
import type { TablePresentation } from '../contracts/table-presentation';
import { realizeProjectedCollection, realizeCollection, type CollectionElement } from './realize-collection';
import { semanticWork } from './semantic-work';

interface ActionResultBase {
  /** Composition role declared by the selected Action Visualizer implementation. */
  role: string;
  label: string;

  transaction: MapTransaction;
  /** Parent and runtime must be bound to this collection's transaction. */
  parentVisualizer: HolonReference;
  materialized: MaterializedVisualizerRuntime;
  isOrdered: boolean;
}

export type ActionResultCollection = ActionResultBase & (
  { collection: DescribedHolonCollection; projection?: never } |
  { collection?: never; projection: { elementType: HolonReference; presentation: TablePresentation; activate(id: string): void } }
);

export interface ActionResultInspection {
  reference: HolonReference;
  source: HTMLElement;
  result: ActionResultCollection;
  origin: ActionBinding;
}

/** Action-owned collection occurrences. Hiding a role retains its live view state;
 * closing the owner revokes all intents without disposing borrowed transactions. */
export class ActionResultCollections {
  readonly element = document.createElement('section');
  private readonly occurrences = new Map<string, {
    binding: ActionResultCollection; tab: HTMLButtonElement; panel: HTMLElement;
    content?: CollectionElement; generation: number; loaded: boolean; unsubscribe: () => void;
  }>();
  private active?: string;
  private disposed = false;

  constructor(private readonly origin: ActionBinding, results: readonly ActionResultCollection[],
    theme: Pick<DahnTheme, 'cssCustomProperties'>, private readonly inspect: (intent: ActionResultInspection) => void) {
    if (new Set(results.map(result => result.role)).size !== results.length) throw new Error('Duplicate result roles');
    this.element.setAttribute('aria-label', 'Action results');
    Object.assign(this.element.style, { display: 'flex', flexDirection: 'column', minHeight: '0', height: '100%', overflow: 'hidden' });
    for (const [name, value] of Object.entries(theme.cssCustomProperties)) this.element.style.setProperty(name, value);
    const tabs = document.createElement('div'); tabs.setAttribute('role', 'tablist'); tabs.setAttribute('aria-label', 'Result collections');
    // A single collection is already selected; its owner supplies the view label.
    tabs.hidden = results.length < 2;
    this.element.append(tabs);
    for (const binding of results) {
      const tab = document.createElement('button'); tab.type = 'button'; tab.textContent = binding.label;
      tab.setAttribute('role', 'tab');
      const panel = document.createElement('div'); panel.setAttribute('role', 'tabpanel'); panel.setAttribute('aria-label', binding.label);
      const id = `action-result-${crypto.randomUUID()}`; panel.id = id; tab.setAttribute('aria-controls', id);
      tab.id = `${id}-tab`; panel.setAttribute('aria-labelledby', tab.id);
      // Collection visualizers need a flex parent to receive the panel's available height.
      Object.assign(panel.style, { display: 'flex', flexDirection: 'column', minHeight: '0', minWidth: '0', overflow: 'auto', flex: '1 1 auto' });
      tab.addEventListener('click', () => this.show(binding.role));
      tab.addEventListener('keydown', event => {
        const roles = [...this.occurrences.keys()]; const index = roles.indexOf(binding.role);
        const next = event.key === 'ArrowRight' ? roles[(index + 1) % roles.length]
          : event.key === 'ArrowLeft' ? roles[(index + roles.length - 1) % roles.length]
          : event.key === 'Home' ? roles[0] : event.key === 'End' ? roles.at(-1) : undefined;
        if (next) { event.preventDefault(); this.show(next); this.occurrences.get(next)!.tab.focus(); }
      });
      const occurrence = { binding, tab, panel, generation: 0, loaded: false, content: undefined as CollectionElement | undefined, unsubscribe: () => {} };
      occurrence.unsubscribe = semanticWork(binding.transaction).onInvalidate(() => {
        ++occurrence.generation; occurrence.loaded = false;
        occurrence.content?.setInspectHolonHandler(null); occurrence.content?.setActivateRowHandler?.(null); occurrence.content = undefined;
        panel.textContent = 'Semantic context changed. Select the collection again.';
      });
      this.occurrences.set(binding.role, occurrence); tabs.append(tab); this.element.append(panel);
    }
    if (results.length) this.show(results[0].role);
  }

  show(role: string): void {
    if (this.disposed) return;
    const occurrence = this.occurrences.get(role);
    if (!occurrence) throw new Error(`Unknown result role ${role}`);
    this.active = role;
    for (const [key, item] of this.occurrences) {
      item.panel.hidden = key !== role;
      item.panel.style.display = key === role ? 'flex' : 'none';
      item.tab.setAttribute('aria-selected', String(key === role));
      item.tab.tabIndex = key === role ? 0 : -1;
    }
    if (occurrence.loaded) return;
    occurrence.loaded = true;
    const generation = ++occurrence.generation;
    const { binding, panel } = occurrence;
    const work = semanticWork(binding.transaction); const revision = work.revision;
    const current = () => !this.disposed && occurrence.generation === generation && work.revision === revision;
    panel.textContent = `Opening ${binding.label}…`;
    void work.realize(async () => {
      let stage = 'Slot ownership';
      try {
        if (!current()) return;
        const slot = await binding.materialized.slot(binding.parentVisualizer, binding.role);
        if (!current()) return;
        const element = binding.projection
          ? await realizeProjectedCollection(binding.transaction, binding.projection.elementType, binding.parentVisualizer,
            slot, binding.materialized, current, name => { stage = name; })
          : await realizeCollection(binding.transaction, binding.collection, binding.parentVisualizer,
            slot, binding.materialized, current, name => { stage = name; });
        if (!element || !current()) return;
        stage = 'Collection presentation';
        if (binding.projection) {
          element.setProjection!(binding.projection.presentation);
          element.setActivateRowHandler!(id => {
            if (current() && this.active === role && element.isConnected) binding.projection!.activate(id);
          });
        } else await element.setCollection(binding.collection, binding.label, { isOrdered: binding.isOrdered });
            await bindCollectionVisualizerInformation(element);
        if (!current()) return;
        element.setInspectHolonHandler(reference => {
          if (current() && this.active === role && element.isConnected) this.inspect({ reference, source: element, result: binding, origin: this.origin });
        });
        occurrence.content = element; panel.replaceChildren(element);
      } catch (error) {
        if (!current()) return;
        occurrence.loaded = false;
        panel.textContent = `${stage}: ${error instanceof Error ? error.message : String(error)}`;
        const retry = document.createElement('button'); retry.type = 'button'; retry.textContent = 'Retry';
        retry.addEventListener('click', () => { if (current()) this.show(role); }); panel.append(retry);
      }
    });
  }

  dispose(): void {
    this.disposed = true;
    for (const occurrence of this.occurrences.values()) {
      ++occurrence.generation; occurrence.unsubscribe(); occurrence.content?.setInspectHolonHandler(null); occurrence.content?.setActivateRowHandler?.(null);
    }
    this.occurrences.clear(); this.element.remove();
  }
}
