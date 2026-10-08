import { afterEach, expect, it } from 'vitest';
import { bindVisualizerInformationControl, selectedVisualizerInspection, withVisualizerComposition, VISUALIZER_INFORMATION_EVENT } from './visualizer-information-control';
import type { VisualizerElement, VisualizerInspectionTarget } from '../contracts/visualizers';
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

it.each(['explicit', 'owned'])('uses actual selected children and implementation-owned structure (%s metadata)', async mode => {
  const parent = document.createElement('div') as VisualizerElement;
  const group = document.createElement('section') as VisualizerElement, child = document.createElement('div');
  parent.append(group); group.append(child); document.body.append(parent);
  const transaction = {} as MapTransaction, subject = {} as HolonReference, slot = {} as HolonReference;
  const definition = (name: string) => ({ propertyValue: async () => ({ StringValue: name }) }) as HolonReference;
  const containing = definition('Actions'), selected = definition('Action');
  const children = [{ label: 'Run', element: child }];
  parent.getVisualizerComposition = () => [{ label: 'Group', element: group, children: mode === 'explicit' ? children : undefined }];
  if (mode === 'owned') group.getVisualizerComposition = () => children;
  await bindVisualizerInformationControl(parent, transaction, subject, containing, slot, containing);
  await bindVisualizerInformationControl(child, transaction, subject, containing, slot, selected);
  const root = withVisualizerComposition(selectedVisualizerInspection(parent)!);
  const entry = root.composition!()[0];
  expect(entry.ownership).toBe('implementation');
  const owned = entry.inspect()!;
  expect(owned.selectedVisualizer).toBe(containing);
  expect(owned.regionLabel).toBe('Group');
  expect(owned.occurrenceId).not.toBe(root.occurrenceId);
  const nested = owned.composition!()[0];
  expect(nested.ownership).toBe('selected');
  expect(nested.inspect()!.selectedVisualizer).toBe(selected);
  expect(nested.inspect()!.occurrenceId).not.toBe(owned.occurrenceId);
  const oldTarget = nested.inspect()!;
  child.remove();
  const replacement = document.createElement('div'); group.append(replacement);
  await bindVisualizerInformationControl(replacement, transaction, subject, containing, slot, selected);
  expect(oldTarget.isLive()).toBe(false);
  expect(nested.inspect()).toBeUndefined();
  expect(selectedVisualizerInspection(replacement)!.occurrenceId).not.toBe(oldTarget.occurrenceId);
  document.body.append(group);
  expect(owned.isLive()).toBe(false);
  expect(root.composition!()).toHaveLength(0);
});
