import { afterEach, expect, it, vi } from 'vitest';
import { ExplorationTabs, type ExplorationPresentation } from './exploration-tabs';
import type { HolonReference } from '../deps';
import type { VisualizerElement } from '../contracts/visualizers';

const anchor = {} as HolonReference;
const pending = <T>() => {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};
function presentation(title: string) {
  const element = document.createElement('div') as unknown as VisualizerElement;
  element.dataset['root'] = title;
  element.requestView = vi.fn(() => true);
  element.requestAttention = vi.fn(() => ({ status: 'applied' }));
  return { title, element, dispose: vi.fn(() => element.remove()) };
}
let tabs: ExplorationTabs;
afterEach(() => { tabs?.dispose(); document.body.replaceChildren(); });
const buttons = () => [...tabs.querySelectorAll<HTMLButtonElement>('[role="tab"]')];
const active = () => tabs.querySelector('[aria-selected="true"]')?.textContent;
const click = (text: string) => [...tabs.querySelectorAll<HTMLButtonElement>('button')].find(b => b.textContent === text)!.click();

it('keeps independent connected presentations and forwards view requests only to the active tab', async () => {
  const a = presentation('HolonSpace'), c = presentation('C');
  const factory = vi.fn().mockResolvedValueOnce(a).mockResolvedValueOnce(c);
  tabs = new ExplorationTabs(factory); document.body.append(tabs);
  await tabs.open(anchor);
  const state = document.createElement('input'); state.value = 'retained selection'; a.element.append(state);
  a.element.scrollLeft = 123;
  await tabs.open(anchor);
  expect(active()).toBe('C');
  expect(tabs.querySelector('[aria-label="Close exploration C"]')?.textContent).toBe('×');
  expect(a.element.isConnected).toBe(true);
  expect(a.dispose).not.toHaveBeenCalled();
  tabs.requestView('actual-size');
  expect(c.element.requestView).toHaveBeenCalledWith('actual-size');
  expect(a.element.requestView).not.toHaveBeenCalled();
  buttons()[0].click();
  expect(active()).toBe('HolonSpace');
  expect(a.element.scrollLeft).toBe(123);
  expect(state.value).toBe('retained selection');
  expect(factory).toHaveBeenCalledTimes(2);
  expect(buttons()[0].getAttribute('aria-controls')).not.toBe(buttons()[1].getAttribute('aria-controls'));
});

it('acknowledges pending work without changing active exploration and activates a usable root', async () => {
  const ready = pending<ExplorationPresentation>();
  tabs = new ExplorationTabs(vi.fn().mockResolvedValueOnce(presentation('A')).mockReturnValueOnce(ready.promise));
  document.body.append(tabs); await tabs.open(anchor);
  const opening = tabs.open(anchor);
  expect(active()).toBe('A');
  expect(tabs.querySelector('[aria-busy="true"]')).not.toBeNull();
  ready.resolve(presentation('C')); await opening;
  expect(active()).toBe('C');
});

it('does not steal activation after the user switches tabs while loading', async () => {
  const ready = pending<ExplorationPresentation>();
  tabs = new ExplorationTabs(vi.fn().mockResolvedValueOnce(presentation('A')).mockResolvedValueOnce(presentation('B')).mockReturnValueOnce(ready.promise));
  document.body.append(tabs); await tabs.open(anchor); await tabs.open(anchor);
  const opening = tabs.open(anchor);
  buttons()[0].click();
  ready.resolve(presentation('C')); await opening;
  expect(active()).toBe('A');
  expect(buttons()[2].dataset['ready']).toBe('true');
  buttons()[2].click(); expect(active()).toBe('C');
});

it('removes failed destinations, preserves the source, and retries through the same anchor', async () => {
  const a = presentation('A');
  const factory = vi.fn().mockResolvedValueOnce(a).mockRejectedValueOnce(new Error('Selection refused')).mockResolvedValueOnce(presentation('C'));
  tabs = new ExplorationTabs(factory); document.body.append(tabs); await tabs.open(anchor); await tabs.open(anchor);
  expect(active()).toBe('A'); expect(buttons()).toHaveLength(1);
  expect(tabs.querySelector('[role="alert"]')?.textContent).toContain('Selection refused');
  expect(a.dispose).not.toHaveBeenCalled();
  click('Retry');
  await vi.waitFor(() => expect(active()).toBe('C'));
  expect(factory.mock.calls.every(call => call[0] === anchor)).toBe(true);
  expect(tabs.querySelector('[role="alert"]')).toBeNull();
});

it('dismisses errors without affecting the source', async () => {
  tabs = new ExplorationTabs(vi.fn().mockResolvedValueOnce(presentation('A')).mockRejectedValueOnce(new Error('Unavailable')));
  document.body.append(tabs); await tabs.open(anchor); await tabs.open(anchor); click('Dismiss');
  expect(tabs.querySelector('[role="alert"]')).toBeNull(); expect(active()).toBe('A');
});

it('cancels pending work and disposes its late result without attachment or activation', async () => {
  const ready = pending<ExplorationPresentation>();
  const factory = vi.fn().mockResolvedValueOnce(presentation('A')).mockReturnValueOnce(ready.promise);
  tabs = new ExplorationTabs(factory); document.body.append(tabs); await tabs.open(anchor);
  const opening = tabs.open(anchor); tabs.querySelector<HTMLButtonElement>('[aria-label="Cancel pending exploration"]')!.click();
  expect((factory.mock.calls[1][1] as AbortSignal).aborted).toBe(true);
  const c = presentation('C'); ready.resolve(c); await opening;
  expect(c.dispose).toHaveBeenCalledTimes(1); expect(c.element.isConnected).toBe(false);
  expect(active()).toBe('A'); expect(buttons()).toHaveLength(1);
});

it('closing the active tab recovers a sibling and releases only its own presentation', async () => {
  const a = presentation('A'), c = presentation('C');
  tabs = new ExplorationTabs(vi.fn().mockResolvedValueOnce(a).mockResolvedValueOnce(c));
  document.body.append(tabs); await tabs.open(anchor); await tabs.open(anchor);
  tabs.querySelector<HTMLButtonElement>('[aria-label="Close exploration C"]')!.click();
  expect(c.dispose).toHaveBeenCalledTimes(1); expect(a.dispose).not.toHaveBeenCalled(); expect(active()).toBe('A');
  tabs.dispose(); expect(a.dispose).toHaveBeenCalledTimes(1);
});

it('consumes late rejection after experience destruction', async () => {
  const ready = pending<ExplorationPresentation>();
  tabs = new ExplorationTabs(() => ready.promise); document.body.append(tabs);
  const opening = tabs.open(anchor); tabs.dispose(); ready.reject(new Error('late')); await opening;
  expect(document.querySelector('[role="alert"]')).toBeNull();
});

it('hides the whole inactive subtree even when descendant visibility is explicit', async () => {
  const a = presentation('A'), b = presentation('B');
  const property = document.createElement('div');
  property.style.visibility = 'visible';
  a.element.append(property);
  tabs = new ExplorationTabs(vi.fn().mockResolvedValueOnce(a).mockResolvedValueOnce(b));
  document.body.append(tabs);
  await tabs.open(anchor); await tabs.open(anchor);
  expect(a.element.parentElement!.style.opacity).toBe('0');
  expect(a.element.isConnected).toBe(true);
  buttons()[0].click();
  expect(a.element.parentElement!.style.opacity).toBe('1');
  expect(b.element.parentElement!.style.opacity).toBe('0');
});
