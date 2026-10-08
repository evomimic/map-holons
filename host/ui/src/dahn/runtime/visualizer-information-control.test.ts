import { afterEach, expect, it } from 'vitest';
import { bindVisualizerInformationControl, VISUALIZER_INFORMATION_EVENT } from './visualizer-information-control';
import type { VisualizerInspectionTarget } from '../contracts/visualizers';
import type { HolonReference, MapTransaction } from '../deps';
afterEach(() => document.body.replaceChildren());

it('captures each slot selection independently and never activates its enclosing action', async () => {
  const parent = document.createElement('div'); document.body.append(parent);
  const owner = {} as HolonReference, subject = {} as HolonReference, transaction = {} as MapTransaction;
  const targets: VisualizerInspectionTarget[] = [];
  parent.addEventListener(VISUALIZER_INFORMATION_EVENT, event => targets.push((event as CustomEvent).detail));
  let actionClicks = 0; parent.addEventListener('click', () => ++actionClicks);
  const choices = ['Properties', 'Value'].map(name => ({ propertyValue: async () => ({ StringValue: name }) }) as HolonReference);
  const slots = [{} as HolonReference, {} as HolonReference];
  for (const [index, selected] of choices.entries()) {
    const element = document.createElement('div'); parent.append(element);
    await bindVisualizerInformationControl(element, transaction, subject, owner, slots[index], selected);
    const button = element.querySelector('button')!;
    expect(button.textContent).toBe('v'); expect(button.style.width).toBe('18px');
    button.focus();
    expect(element.querySelector<HTMLElement>('[role=tooltip]')!.hidden).toBe(false);
    button.click();
  }
  expect(actionClicks).toBe(0);
  expect(targets.map(target => target.selectedVisualizer)).toEqual(choices);
  expect(targets.map(target => target.slot)).toEqual(slots);
  expect(targets[0].occurrenceId).not.toBe(targets[1].occurrenceId);
  expect(targets[0].isLive()).toBe(true);
  targets[0].element.remove(); expect(targets[0].isLive()).toBe(false);
  expect(targets[1].isLive()).toBe(true);
});
