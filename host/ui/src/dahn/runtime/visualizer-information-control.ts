import type { HolonReference, MapTransaction } from '../deps';
import type { VisualizerElement, VisualizerInspectionEntry, VisualizerInspectionTarget, VisualizerPresentationRegion } from '../contracts/visualizers';

export const VISUALIZER_INFORMATION_EVENT = 'dahn-visualizer-information';

const selections = new WeakMap<HTMLElement, () => VisualizerInspectionTarget>();
const regionIdentities = new WeakMap<HTMLElement, string>();

/** Register an owner-bound selection whose invoker is supplied by the implementation. */
export function registerVisualizerInspection(element: HTMLElement, target: () => VisualizerInspectionTarget): void {
  selections.set(element, target);
}

/** Read the selection captured at realization, never reconstruct it from the DOM. */
export function selectedVisualizerInspection(element: HTMLElement): VisualizerInspectionTarget | undefined {
  return selections.get(element)?.();
}

/** Resolve implementation-declared regions against the actual live child selections. */
export function visualizerInspectionEntries(target: VisualizerInspectionTarget, regions: readonly VisualizerPresentationRegion[]): readonly VisualizerInspectionEntry[] {
  return regions.filter(region => region.element.isConnected && target.element.contains(region.element)).map(region => {
    const selected = selectedVisualizerInspection(region.element);
    if (!regionIdentities.has(region.element)) regionIdentities.set(region.element, crypto.randomUUID());
    return {
      label: region.label, ownership: selected ? 'selected' : 'implementation', displayName: selected?.displayName,
      inspect: () => {
        if (!target.isLive() || !region.element.isConnected || !target.element.contains(region.element)) return;
        if (selected) return selected.isLive() ? withVisualizerComposition(selected) : undefined;
        const owned: VisualizerInspectionTarget = {
          ...target, occurrenceId: `${target.occurrenceId}/region-${regionIdentities.get(region.element)}`, element: region.element,
          regionLabel: region.label,
          isLive: () => target.isLive() && region.element.isConnected && target.element.contains(region.element),
          composition: () => visualizerInspectionEntries(owned, region.children
            ?? (region.element as VisualizerElement).getVisualizerComposition?.() ?? []),
        };
        return owned;
      },
    };
  });
}

/** Composition remains occurrence-local and is read afresh when disclosure is opened. */
export function withVisualizerComposition(target: VisualizerInspectionTarget): VisualizerInspectionTarget {
  if (target.composition || !(target.element as VisualizerElement).getVisualizerComposition) return target;
  return { ...target, composition: () => visualizerInspectionEntries(target,
    (target.element as VisualizerElement).getVisualizerComposition?.() ?? []) };
}

/** Revoke nested slot invokers when their containing presentation is retired. */
export function revokeVisualizerInformationControls(element: HTMLElement): void {
  selections.delete(element);
  for (const control of element.querySelectorAll('[data-visualizer-information-control]')) control.remove();
}

/** Decorate an actual selected slot child; no selection is reconstructed by the control. */
export async function bindVisualizerInformationControl(
  element: HTMLElement, transaction: MapTransaction, subject: VisualizerInspectionTarget['subject'],
  owner: HolonReference, slot: HolonReference, selectedVisualizer: HolonReference,
  placement: 'start' | 'end' = 'end',
  regionLabel?: string,
): Promise<void> {
  const value = await selectedVisualizer.propertyValue('DisplayName');
  const displayName = value && 'StringValue' in value ? value.StringValue
    : await selectedVisualizer.key() ?? await selectedVisualizer.versionedKey();
  const container = document.createElement('span');
  container.dataset['visualizerInformationControl'] = 'true';
  container.style.cssText = `position:absolute;top:2px;${placement === 'start' ? 'left' : 'right'}:2px;display:inline-flex;z-index:2;`;
  const button = document.createElement('button'); button.type = 'button'; button.textContent = 'v';
  button.setAttribute('aria-label', `Visualizer information: ${displayName}`);
  button.style.cssText = 'box-sizing:border-box;width:18px;height:18px;min-width:18px;padding:0;border:1px solid currentColor;border-radius:50%;font:12px/16px sans-serif;text-transform:none;color:var(--dahn-action-text-color);background:var(--dahn-action-surface-background);cursor:pointer;';
  const tooltip = document.createElement('span'); tooltip.textContent = displayName;
  tooltip.id = `visualizer-name-${crypto.randomUUID()}`; tooltip.setAttribute('role', 'tooltip');
  button.setAttribute('aria-describedby', tooltip.id); tooltip.hidden = true; tooltip.setAttribute('popover', 'manual');
  tooltip.style.cssText = 'position:absolute;bottom:calc(100% + 4px);right:0;white-space:nowrap;max-width:90vw;padding:6px 8px;border:1px solid var(--dahn-slot-border-color);background:var(--dahn-panel-surface-background);color:var(--dahn-canvas-text-color);font:14px/1.4 sans-serif;pointer-events:none;';
  const show = (visible: boolean) => {
    tooltip.hidden = !visible;
    if (visible && tooltip.showPopover) {
      tooltip.showPopover(); const rect = button.getBoundingClientRect();
      Object.assign(tooltip.style, { position: 'fixed', margin: '0', right: 'auto', left: `${Math.max(4, Math.min(rect.left, window.innerWidth - tooltip.offsetWidth - 4))}px`, top: 'auto', bottom: `${window.innerHeight - rect.top + 4}px` });
    } else if (!visible && tooltip.hidePopover) tooltip.hidePopover();
  };
  for (const name of ['mouseenter', 'focus']) button.addEventListener(name, () => show(true));
  for (const name of ['mouseleave', 'blur']) button.addEventListener(name, () => show(false));
  button.addEventListener('keydown', event => { if (event.key === 'Escape') show(false); });
  const occurrenceId = `visualizer-slot-${crypto.randomUUID()}`;
  const target = (): VisualizerInspectionTarget => ({
    occurrenceId, context: transaction, owner, slot, subject, selectedVisualizer, displayName, regionLabel,
    element, invoker: button, isLive: () => element.isConnected && element.contains(button),
  });
  selections.set(element, target);
  button.addEventListener('click', event => {
    event.stopPropagation(); show(false);
    if (!element.isConnected) return;
    element.dispatchEvent(new CustomEvent<VisualizerInspectionTarget>(VISUALIZER_INFORMATION_EVENT, { bubbles: true, composed: true, detail: withVisualizerComposition(target()) }));
  });
  container.append(button, tooltip);
  element.style.position = 'relative';
  if (placement === 'start') element.style.paddingInlineStart = '24px';
  else element.style.paddingInlineEnd = '24px';
  element.append(container);
}
