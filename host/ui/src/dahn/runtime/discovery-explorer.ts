import { MapClient, type HolonReference, type MapTransaction, type VisualizerDiscovery, type VisualizerUsageSelection } from '../deps';
import type { DiscoveryExplorerState, DiscoveryPresentation, VisualizerContext, VisualizerElement, VisualizerInspectionTarget } from '../contracts/visualizers';
import type { SpaceNavigatorBinding } from './space-navigator-experience';
import { DahnHolonView } from '../map-adapter/dahn-holon-view';
import { SdkVisualizerMaterializer } from '../map-adapter/sdk-visualizer-materializer';
import { MaterializedVisualizerCache } from './materialized-visualizer-cache';
import { defineCustomElementOnce } from '../visualizers/define-custom-element-once';
import { reportSuccessfulVisualizerUse } from './successful-visualizer-use';

const related = async (reference: HolonReference, name: string) => [...await reference.relatedHolons(name)];
async function textValue(reference: HolonReference, property: string): Promise<string | undefined> {
  const value = await reference.propertyValue(property);
  return value && 'StringValue' in value ? value.StringValue : undefined;
}
const name = async (reference: HolonReference) => await textValue(reference, 'DisplayName') ?? await reference.key() ?? await reference.versionedKey();
async function single(reference: HolonReference, relationship: string): Promise<HolonReference> {
  const values = await related(reference, relationship);
  if (values.length !== 1) throw new Error(`Discovery evidence requires one ${relationship}.`);
  return values[0];
}

/** Read only the supplied projection; ordering comes from Rust's captured indices. */
export async function readDiscoveryPresentation(subject: HolonReference): Promise<DiscoveryPresentation> {
  const levels = await Promise.all((await related(subject, 'DiscoveryLevels')).map(async level => {
    const descriptor = await single(level, 'DiscoveryDescriptor');
    const value = await level.propertyValue('DiscoveryLevelIndex');
    const index = value && 'IntegerValue' in value ? value.IntegerValue : undefined;
    if (!Number.isSafeInteger(index) || Number(index) < 0) throw new Error('Discovery level has no valid captured index.');
    return { index: Number(index), descriptor, label: await name(descriptor) };
  }));
  levels.sort((a, b) => a.index - b.index);
  if (levels.some((level, index) => level.index !== index)) throw new Error('Discovery level indices are incomplete.');
  const [current] = await related(subject, 'DiscoveryCurrentVisualizer');
  const candidates = await Promise.all((await related(subject, 'DiscoveryCandidates')).map(async record => {
    const visualizer = await single(record, 'DiscoveryVisualizer');
    const assessment = await textValue(record, 'DiscoveryAssessment');
    if (!assessment) throw new Error('Discovery candidate has no assessment.');
    return { visualizer, label: await name(visualizer), declarations: await related(record, 'DiscoveryDeclarations'), assessment,
      current: current?.equals(visualizer) ?? false };
  }));
  const [endpoint] = await related(subject, 'DiscoveryEndpoint');
  const stopReason = await textValue(subject, 'DiscoveryStopReason');
  if (stopReason !== 'holon_type_boundary' && stopReason !== 'lineage_exhausted') throw new Error('Discovery stopping evidence is unavailable.');
  const request = await Promise.all(['DiscoverySubject', 'DiscoverySlot', 'DiscoveryOwner', 'DiscoveryTheme'].map(async relationship => name(await single(subject, relationship))));
  const kind = await textValue(subject, 'DiscoveryRequestedKind');
  if (!kind) throw new Error('The captured discovery request has no kind.');
  return { levels, candidates, endpoint: endpoint ? await name(endpoint) : 'no captured descriptor', stopReason,
    requestContext: `Captured ${kind} request for ${request[0]}, in ${request[1]}, owned by ${request[2]}, using ${request[3]}.`,
    rationale: await textValue(subject, 'SelectionRationale') ?? 'Selection rationale is unavailable.' };
}

/** The Inspector owns semantic evidence and selection; each mounted realization
 * has a disposable presentation transaction. Suspended sessions retain no DOM. */
export class DiscoveryExplorerOwner {
  readonly state: DiscoveryExplorerState = {};
  private disposed = false;
  private evidenceTransaction?: MapTransaction;
  private ownsEvidenceTransaction = false;
  private subject?: HolonReference;
  private slot?: HolonReference;
  private selected?: HolonReference;
  private usage?: VisualizerUsageSelection;
  private evidence?: DiscoveryPresentation;
  private mounted?: { element: VisualizerElement; transaction: MapTransaction };
  private pending = new Set<Promise<unknown>>();
  private initialization?: Promise<void>;
  private generation = 0;
  private explicitPending = false;
  private allocation = { width: 320, height: 600 };
  private callbacks?: Pick<VisualizerContext, 'inspectVisualizerCandidate' | 'chooseVisualizerCandidate'>;

  constructor(private readonly binding: SpaceNavigatorBinding, private readonly target: VisualizerInspectionTarget,
    private readonly inspector: HolonReference, private discovery: VisualizerDiscovery,
    private readonly inspect: (target: VisualizerInspectionTarget) => void,
    private readonly client = new MapClient()) {}

  private track<T>(promise: Promise<T>): Promise<T> {
    this.pending.add(promise);
    void promise.finally(() => this.pending.delete(promise)).catch(() => {});
    return promise;
  }

  private request() {
    return { subject: this.subject!, requestedKind: 'structure' as const, slot: this.slot!,
      owner: { visualizer: this.inspector }, theme: this.binding.theme.reference };
  }

  private initialize(): Promise<void> {
    return this.initialization ??= this.track((async () => {
      if (!this.discovery.evidence) throw new Error('This discovery result has no retained authoritative evidence. Refresh its alternatives.');
      this.ownsEvidenceTransaction = !this.discovery.evidence.projectionTransaction;
      const transaction = this.evidenceTransaction = this.discovery.evidence.projectionTransaction ?? await this.client.beginTransaction();
      if (this.disposed) return;
      this.subject = await this.discovery.evidence.project(transaction);
      this.evidence = await readDiscoveryPresentation(this.subject);
      // The parent Inspector is already materialized in the experience cache.
      // Its composition declaration requires no second artifact or private pool.
      this.slot = await this.binding.materialized.slot(transaction.bindSavedReference(this.inspector), 'discovery');
      this.selected = (await transaction.selectVisualizer(this.request())).selected;
      this.usage = await transaction.findVisualizerUsage(this.request(), this.selected) ?? undefined;
    })());
  }

  private async prepare(selected: HolonReference, usage?: VisualizerUsageSelection): Promise<{ element: VisualizerElement; transaction: MapTransaction }> {
    const transaction = await this.client.beginTransaction();
    let element: VisualizerElement | undefined;
    try {
      if (this.disposed) throw new Error('The Visualizer Inspector was disposed.');
      const cache = new MaterializedVisualizerCache(new SdkVisualizerMaterializer(transaction));
      const implementation = await this.binding.materialized.realize(transaction.bindSavedReference(selected), cache);
      if (typeof implementation !== 'function' || !(implementation.prototype instanceof HTMLElement)) throw new Error('Selected explorer is not an HTMLElement constructor.');
      element = document.createElement(defineCustomElementOnce('map-discovery-explorer', implementation as CustomElementConstructor)) as VisualizerElement;
      if (!element.setSpatialBudget || !element.setVisualizerInformationHandler || !element.dispose) throw new Error('Selected explorer does not fulfill its Structure slot participation contract.');
      element.setContext({ target: { reference: this.subject! }, holon: new DahnHolonView(this.subject!),
        discoveryExplorer: { evidence: this.evidence!, state: this.state }, visualizerUsage: usage?.usage,
        ...this.callbacks, actions: [], theme: this.binding.theme, canvas: this.binding.canvas });
      if (!element.ready) throw new Error('Selected explorer has no explicit initialization signal.');
      await element.ready;
      element.setSpatialBudget(this.allocation);
      if (this.disposed) throw new Error('The Visualizer Inspector was disposed.');
      return { element, transaction };
    } catch (error) {
      try { element?.dispose?.(); }
      finally { element?.remove(); await transaction.dispose(); }
      throw error;
    }
  }

  async mount(host: HTMLElement, callbacks: Pick<VisualizerContext, 'inspectVisualizerCandidate' | 'chooseVisualizerCandidate'>): Promise<void> {
    await this.track((async () => {
      const generation = ++this.generation;
      await this.initialize();
      if (this.disposed || !this.target.isLive() || !host.isConnected || generation !== this.generation) return;
      this.callbacks = callbacks;
      this.allocation = { width: host.clientWidth || 320, height: 600 };
      const mounted = await this.prepare(this.selected!, this.usage);
      if (this.disposed || !this.target.isLive() || !host.isConnected || generation !== this.generation) {
        try { mounted.element.dispose?.(); }
        finally { mounted.element.remove(); await mounted.transaction.dispose(); }
        return;
      }
      await this.releaseMounted();
      this.mounted = mounted;
      host.replaceChildren(mounted.element);
      const label = await name(this.selected!);
      if (this.disposed || !this.target.isLive() || generation !== this.generation || this.mounted !== mounted) return;
      mounted.element.setVisualizerInformationHandler?.(invoker => this.inspect(this.inspectionTarget(invoker, mounted.element)), label);
      if (this.explicitPending) {
        this.explicitPending = false;
        if (this.usage) reportSuccessfulVisualizerUse(this.evidenceTransaction!, this.request(), this.selected!, this.usage, `${this.target.occurrenceId}:discovery`);
      }
    })());
  }

  private inspectionTarget(invoker: HTMLElement, element: HTMLElement): VisualizerInspectionTarget {
    const selected = this.selected!;
    return { occurrenceId: `${this.target.occurrenceId}:discovery`, context: this, owner: this.inspector,
      slot: this.slot!, subject: this.subject!, selectedVisualizer: selected, element, invoker,
      isLive: () => !this.disposed && this.target.isLive() && this.selected?.equals(selected) === true,
      choices: {
        discover: () => this.evidenceTransaction!.discoverVisualizers(this.request(), this.selected, true),
        choose: (candidate, current, signal) => this.choose(candidate, current, signal, invoker),
      } };
  }

  private choose(candidate: HolonReference, current: () => boolean, signal: AbortSignal | undefined, invoker: HTMLElement): Promise<VisualizerInspectionTarget> {
    return this.track((async () => {
      const generation = this.generation;
      const valid = () => !this.disposed && !signal?.aborted && current() && this.target.isLive() && generation === this.generation;
      if (!valid()) throw new Error('Explorer choice is closed or superseded.');
      const selected = (await this.evidenceTransaction!.chooseVisualizer(this.request(), candidate)).selected;
      const usage = await this.evidenceTransaction!.selectVisualizerUsage(this.request(), selected);
      const prepared = await this.prepare(selected, usage);
      try {
        if (!valid()) throw new Error('Explorer choice is closed or superseded.');
        this.selected = selected; this.usage = usage; this.explicitPending = true;
        // The invoking Inspector is suspended. Publish its semantic selection,
        // then realize/report presentation when Back restores that owner.
        return this.inspectionTarget(invoker, prepared.element);
      } finally {
        try { prepared.element.dispose?.(); }
        finally { prepared.element.remove(); await prepared.transaction.dispose(); }
      }
    })());
  }

  private async releaseMounted(): Promise<void> {
    const mounted = this.mounted; this.mounted = undefined;
    if (mounted) {
      try { mounted.element.dispose?.(); }
      finally { mounted.element.remove(); await mounted.transaction.dispose(); }
    }
  }

  suspend(): void {
    ++this.generation;
    this.callbacks = undefined;
    void this.track(this.releaseMounted()).catch(console.error);
  }

  refresh(discovery: VisualizerDiscovery): Promise<void> {
    return this.track(this.refreshEvidence(discovery));
  }

  private async refreshEvidence(discovery: VisualizerDiscovery): Promise<void> {
    if (this.disposed) { await discovery.evidence?.dispose(); return; }
    this.suspend();
    await Promise.allSettled([...this.pending]);
    const previous = this.selected;
    await Promise.allSettled([this.discovery.evidence?.dispose(), this.ownsEvidenceTransaction ? this.evidenceTransaction?.dispose() : undefined]);
    this.discovery = discovery;
    this.initialization = undefined; this.evidenceTransaction = undefined;
    if (this.disposed) return;
    await this.initialize();
    if (this.disposed) return;
    if (previous) {
      const choices = await this.evidenceTransaction!.discoverVisualizers(this.request(), previous);
      if (choices.candidates.some(candidate => candidate.assessment === 'viable' && candidate.visualizer.equals(previous))) {
        this.selected = (await this.evidenceTransaction!.chooseVisualizer(this.request(), previous)).selected;
        this.usage = await this.evidenceTransaction!.findVisualizerUsage(this.request(), this.selected) ?? undefined;
      }
    }
    const present = (reference: HolonReference) => this.evidence!.levels.some(level => level.descriptor.equals(reference))
      || this.evidence!.candidates.some(candidate => candidate.visualizer.equals(reference));
    this.state.notice = undefined;
    if (this.state.selected && !present(this.state.selected)) {
      this.state.selected = undefined; this.state.notice = 'The previously selected declaration or candidate is absent from the refreshed evidence.';
    }
    if (this.state.preview && !this.evidence!.candidates.some(candidate => candidate.visualizer.equals(this.state.preview!))) {
      this.state.preview = undefined; this.state.notice = 'The previously inspected candidate is absent from the refreshed evidence.';
    }
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true; ++this.generation; this.callbacks = undefined;
    void (async () => {
      await Promise.allSettled([...this.pending]);
      await Promise.allSettled([this.releaseMounted(), this.discovery.evidence?.dispose(), this.ownsEvidenceTransaction ? this.evidenceTransaction?.dispose() : undefined]);
      this.evidence = undefined; this.subject = undefined;
    })().catch(error => console.warn('[DAHN] Inspector resource release failed', error));
  }
}
