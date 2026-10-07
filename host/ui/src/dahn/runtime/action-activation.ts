import type { RealizedNode } from './realize-node';
import type { InspectHolonIntent } from '../contracts/visualizers';
import type { ExplorationPresentation } from './exploration-tabs';
import type { HolonReference, MapTransaction } from '../deps';

/** Captured semantic and occurrence identity; navigation never mutates this binding. */
export interface ActionBinding {
  readonly subject: HolonReference;
  readonly dance: HolonReference;
  /** Selected presentation owner, retained for owned result-slot composition. */
  readonly visualizer: HolonReference;
  readonly occurrence: HTMLElement;
  readonly label: string;
  readonly mountPresentation?: (element: HTMLElement, owner: ActionInteraction) => { focus(): void; remove(): void };
  readonly presentResult?: (request: {
    transaction: MapTransaction; review: MapTransaction; subject: HolonReference;
    contextFor?: (subject: HolonReference) => MapTransaction;
    children?: Map<string, HTMLElement>; collections?: RealizedNode['collectionActivation']; signal: AbortSignal;
  }) => Promise<ExplorationPresentation & { inspect(intent: InspectHolonIntent): void }>;
  readonly refreshAfterPersistence?: () => void;
}

export interface ActionInteraction {
  canDismiss(): boolean;
  dispose(): Promise<void>;
  focus(): void;
  readonly closed: Promise<void>;
}

/** Explicit host adapter for the selected loader's Angular review presentation. */
export interface ActionInteractions {
  openLoadHolons(binding: ActionBinding): ActionInteraction;
}

/** Generic occurrence lifetime. Selected Action Visualizers supply the interaction factory. */
export class ActionActivation {
  private interaction?: ActionInteraction;
  private disposed = false;
  constructor(readonly binding: ActionBinding) {}

  activate(open: (binding: ActionBinding) => ActionInteraction): void {
    if (this.disposed) return;
    if (this.interaction) { this.interaction.focus(); return; }
    const interaction = open(this.binding);
    this.interaction = interaction;
    void interaction.closed.then(() => { if (this.interaction === interaction) this.interaction = undefined; });
  }

  canDismiss(): boolean { return this.interaction?.canDismiss() ?? true; }

  async dispose(): Promise<void> {
    if (!this.canDismiss()) throw new Error('An action is executing in this presentation.');
    this.disposed = true;
    await this.interaction?.dispose();
    this.interaction = undefined;
  }
}
