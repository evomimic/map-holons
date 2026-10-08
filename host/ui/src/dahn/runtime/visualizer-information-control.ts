import type { HolonReference, MapTransaction } from '../deps';
import type { VisualizerInspectionTarget } from '../contracts/visualizers';

export const VISUALIZER_INFORMATION_EVENT = 'dahn-visualizer-information';

/** Revoke nested slot invokers when their containing presentation is retired. */
export function revokeVisualizerInformationControls(element: HTMLElement): void {
  for (const control of element.querySelectorAll('[data-visualizer-information-control]')) control.remove();
}

/** Decorate an actual selected slot child; no selection is reconstructed by the control. */
export async function bindVisualizerInformationControl(
  element: HTMLElement, transaction: MapTransaction, subject: VisualizerInspectionTarget['subject'],
  owner: HolonReference, slot: HolonReference, selectedVisualizer: HolonReference,
  placement: 'start' | 'end' = 'end',
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
  button.addEventListener('click', event => {
    event.stopPropagation(); show(false);
    if (!element.isConnected) return;
    element.dispatchEvent(new CustomEvent<VisualizerInspectionTarget>(VISUALIZER_INFORMATION_EVENT, { bubbles: true, composed: true, detail: {
      occurrenceId, context: transaction, owner, slot, subject, selectedVisualizer,
      element, invoker: button, isLive: () => element.isConnected && element.contains(button),
    } }));
  });
  container.append(button, tooltip);
  element.style.position = 'relative';
  if (placement === 'start') element.style.paddingInlineStart = '24px';
  else element.style.paddingInlineEnd = '24px';
  element.append(container);
}
