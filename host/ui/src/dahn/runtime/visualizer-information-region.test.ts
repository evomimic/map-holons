import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { VisualizerInformationRegion } from './visualizer-information-region';
import type { VisualizerInspectionTarget } from '../contracts/visualizers';
import type { SpaceNavigatorBinding } from './space-navigator-experience';

class Inspector extends HTMLElement {
  setContext(context: unknown) { (this as any).context = context; }
}
const ref = (key: string) => ({ key: async () => key });
function fixture(explore?: (reference: VisualizerInspectionTarget['selectedVisualizer']) => void) {
  const host = document.createElement('section');
  const source = document.createElement('div'), invoker = document.createElement('button');
  source.append(invoker); host.append(source); document.body.append(host);
  let live = true;
  const selected = ref('Selected definition');
  const binding = {
    transaction: { owns: () => true, selectVisualizer: vi.fn(async () => ({ selected: ref('Custom Inspector') })), stageNewHolon: vi.fn(), commit: vi.fn(), abandon: vi.fn() },
    dancer: { relatedHolons: async () => [ref('SpaceNavigator.VisualizerInformationSlot')] },
    materialized: { realize: vi.fn(async () => Inspector) }, theme: { reference: ref("Theme") }, canvas: {}, holonSpace: ref('Space'),
  } as unknown as SpaceNavigatorBinding;
  const region = new VisualizerInformationRegion(binding, host, explore);
  host.append(region.element, region.toggle);
  const target = { occurrenceId: 'A', context: host, owner: ref('Owner'), slot: ref('Node slot'), subject: ref('Subject'), selectedVisualizer: selected,
    element: source, invoker, isLive: () => live && source.isConnected } as unknown as VisualizerInspectionTarget;
  return { host, region, binding, target, closeTarget: () => { live = false; source.remove(); } };
}
beforeEach(() => { vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} })); });
afterEach(() => { document.body.replaceChildren(); vi.unstubAllGlobals(); });

it('marks only the inspected occurrence while information is visible', () => {
  const f = fixture(); f.region.inspect(f.target);
  expect(f.target.invoker.getAttribute('aria-pressed')).toBe('true');
  expect(f.target.invoker.dataset['visualizerInspected']).toBe('true');
  f.region.toggle.click();
  expect(f.target.invoker.getAttribute('aria-pressed')).toBe('false');
  f.region.toggle.click();
  expect(f.target.invoker.getAttribute('aria-pressed')).toBe('true');
  const secondInvoker = document.createElement('button'); f.host.append(secondInvoker);
  f.region.inspect({ ...f.target, occurrenceId: 'B', invoker: secondInvoker });
  expect(f.target.invoker.getAttribute('aria-pressed')).toBe('false');
  expect(secondInvoker.getAttribute('aria-pressed')).toBe('true');
  f.region.dismiss();
  expect(secondInvoker.getAttribute('aria-pressed')).toBe('false');
  f.region.dispose();
});

it('explores the captured definition and revokes exploration from replaced inspectors', async () => {
  const explore = vi.fn(), f = fixture(explore); f.region.inspect(f.target);
  await vi.waitFor(() => expect(f.region.element.querySelector('map-visualizer-inspector')).not.toBeNull());
  const callback = (f.region.element.querySelector('map-visualizer-inspector') as any).context.onExploreVisualizer;
  callback(); expect(explore).toHaveBeenCalledWith(f.target.selectedVisualizer);
  f.region.dismiss(); callback(); expect(explore).toHaveBeenCalledTimes(1);
  f.region.dispose();
});

it('realizes nested information in its disclosure without replacing or highlighting away from the parent', async () => {
  const explore = vi.fn(), f = fixture(explore); f.region.inspect(f.target);
  await vi.waitFor(() => expect(f.region.element.querySelector('map-visualizer-inspector')).not.toBeNull());
  const parent = f.region.element.querySelector('map-visualizer-inspector') as any;
  const host = document.createElement('div'); parent.append(host);
  const child = { ...f.target, occurrenceId: 'child', selectedVisualizer: ref('Child definition') } as unknown as VisualizerInspectionTarget;
  await parent.context.mountVisualizerInformation(child, host);
  const nested = host.querySelector('map-visualizer-inspector') as any;
  expect(nested.context.visualizerInspection).toBe(child);
  expect(f.binding.transaction.selectVisualizer).toHaveBeenLastCalledWith({ subject: child.selectedVisualizer, requestedKind: 'node', slot: expect.any(Object), owner: { dancer: f.binding.dancer }, theme: f.binding.theme.reference });
  expect(parent.isConnected).toBe(true);
  expect(parent.context.visualizerInspection).toBe(f.target);
  expect(f.target.invoker.dataset['visualizerInspected']).toBe('true');
  nested.context.onExploreVisualizer(); expect(explore).toHaveBeenCalledWith(child.selectedVisualizer);
  const mount = parent.context.mountVisualizerInformation, count = vi.mocked(f.binding.transaction.selectVisualizer).mock.calls.length;
  f.region.dismiss();
  await mount(child, host);
  nested.context.onExploreVisualizer();
  expect(f.binding.transaction.selectVisualizer).toHaveBeenCalledTimes(count);
  expect(explore).toHaveBeenCalledTimes(1);
  f.region.dispose();
});

it('selects a custom definition presentation through the Dancer slot and retains its captured target', async () => {
  const f = fixture(); f.region.inspect(f.target);
  await vi.waitFor(() => expect(f.region.element.querySelector('map-visualizer-inspector')).not.toBeNull());
  const inspector = f.region.element.querySelector('map-visualizer-inspector') as any;
  expect(inspector.context.target.reference).toBe(f.target.selectedVisualizer);
  expect(inspector.context.visualizerInspection).toBe(f.target);
  const width = f.region.element.style.width;
  f.region.toggle.focus();
  expect(inspector.context.visualizerInspection).toBe(f.target);
  expect(f.region.element.style.width).toBe(width);
  expect(f.binding.transaction.selectVisualizer).toHaveBeenCalledWith({ subject: f.target.selectedVisualizer, requestedKind: 'node', slot: expect.any(Object), owner: { dancer: f.binding.dancer }, theme: f.binding.theme.reference });
  for (const name of ['stageNewHolon', 'commit', 'abandon']) expect((f.binding.transaction as any)[name]).not.toHaveBeenCalled();
  f.region.dispose();
});

it('dismisses a closed target and returns focus to the surviving Information control', async () => {
  const f = fixture(); f.region.inspect(f.target);
  f.closeTarget();
  await vi.waitFor(() => expect(f.region.element.textContent).toContain('Select a Visualizer'));
  expect(document.activeElement).toBe(f.region.toggle);
  expect(f.region.element.style.display).toBe('block');
  f.region.dispose();
});

it('ignores late materialization from an earlier invocation', async () => {
  const f = fixture();
  let resolve!: (value: typeof Inspector) => void;
  vi.mocked(f.binding.materialized.realize).mockImplementationOnce(() => new Promise(done => { resolve = done; }));
  f.region.inspect(f.target);
  await vi.waitFor(() => expect(resolve).toBeTypeOf('function'));
  const second = { ...f.target, occurrenceId: 'B', selectedVisualizer: ref('Second definition') } as unknown as VisualizerInspectionTarget;
  f.region.inspect(second); resolve(Inspector);
  await vi.waitFor(() => expect((f.region.element.querySelector('map-visualizer-inspector') as any)?.context.visualizerInspection).toBe(second));
  expect(f.region.element.querySelectorAll('map-visualizer-inspector')).toHaveLength(1);
  f.region.dispose();
});

it('returns focus to the invoker on Escape and collapses the region', async () => {
  const f = fixture(); f.region.inspect(f.target);
  f.region.element.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  expect(document.activeElement).toBe(f.target.invoker);
  expect(f.region.element.style.display).toBe('none');
  f.region.dispose();
});

it('reports realization failure without removing the source and allows retry', async () => {
  const f = fixture(); vi.mocked(f.binding.materialized.realize).mockRejectedValueOnce(new Error('Unavailable'));
  f.region.inspect(f.target);
  await vi.waitFor(() => expect(f.region.element.querySelector('[role=alert]')?.textContent).toContain('Unavailable'));
  expect(f.target.element.isConnected).toBe(true);
  [...f.region.element.querySelectorAll('button')].find(button => button.textContent === 'Retry')!.click();
  await vi.waitFor(() => expect(f.region.element.querySelector('map-visualizer-inspector')).not.toBeNull());
  f.region.dispose();
});

it('presents a mobile dialog and restores navigation when dismissed', () => {
  vi.stubGlobal('matchMedia', () => ({ matches: true, addEventListener() {}, removeEventListener() {} }));
  const f = fixture(); f.target.element.dataset['explorationNavigation'] = 'true';
  f.region.inspect(f.target);
  expect(f.region.element.getAttribute('role')).toBe('dialog');
  expect(f.region.element.getAttribute('aria-modal')).toBe('true');
  expect(f.target.element.inert).toBe(true);
  f.region.element.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  expect(f.target.element.inert).toBe(false);
  expect(f.region.element.style.display).toBe('none');
  expect(document.activeElement).toBe(f.target.invoker);
  f.region.dispose();
});

it('keeps mobile focus within visible information controls when nested disclosures are closed', () => {
  vi.stubGlobal('matchMedia', () => ({ matches: true, addEventListener() {}, removeEventListener() {} }));
  const f = fixture(); f.region.inspect(f.target);
  const closed = document.createElement('details'), summary = document.createElement('summary'), nested = document.createElement('button');
  summary.textContent = 'Presentation structure'; closed.append(summary, nested); f.region.element.append(closed);
  summary.focus();
  summary.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true }));
  expect(document.activeElement).toBe(f.region.element.querySelector('button'));
  closed.open = true; nested.focus();
  nested.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true }));
  expect(document.activeElement).toBe(f.region.element.querySelector('button'));
  f.region.dispose();
});

it('keeps candidate inspection read-only and binds choice to the captured live session', async () => {
  const f = fixture();
  const candidate = ref('Alternative') as any;
  const discover = vi.fn(async () => ({ candidates: [], currentSelection: null, ancestry: [] }));
  let allowed!: () => boolean;
  const choose = vi.fn(async (_candidate, current) => { allowed = current; throw new Error('admission failed'); });
  const target = { ...f.target, choices: { discover, choose } };
  f.region.inspect(target);
  await vi.waitFor(() => expect((f.region.element.querySelector('map-visualizer-inspector') as any)?.context.discoverVisualizerChoices).toBeTypeOf('function'));
  const inspector = f.region.element.querySelector('map-visualizer-inspector') as any;
  await inspector.context.discoverVisualizerChoices(); expect(discover).toHaveBeenCalledOnce();
  const preview = document.createElement('div'); inspector.append(preview);
  await inspector.context.inspectVisualizerCandidate(candidate, preview);
  const nested = preview.querySelector('map-visualizer-inspector') as any;
  expect(nested.context.visualizerInspection.selectedVisualizer).toBe(candidate);
  expect(nested.context.chooseVisualizerCandidate).toBeUndefined(); expect(choose).not.toHaveBeenCalled();
  const controller = new AbortController();
  await expect(inspector.context.chooseVisualizerCandidate(candidate, controller.signal)).rejects.toThrow('admission failed');
  expect(allowed()).toBe(true); controller.abort(); expect(allowed()).toBe(false);
  f.region.dismiss();
  await expect(inspector.context.discoverVisualizerChoices()).rejects.toThrow('no longer live');
  expect(discover).toHaveBeenCalledOnce(); f.region.dispose();
});

it('refreshes a published choice under its retained occurrence without dismissing the information session', async () => {
  const f = fixture(); const source = document.createElement('div'), invoker = document.createElement('button'); source.append(invoker);
  const replacement = { ...f.target, selectedVisualizer: ref('Alternative'), element: source, invoker,
    isLive: () => source.isConnected } as unknown as VisualizerInspectionTarget;
  const choose = vi.fn(async () => { f.target.element.replaceWith(source); return replacement; });
  f.region.inspect({ ...f.target, choices: { discover: vi.fn(), choose } });
  await vi.waitFor(() => expect((f.region.element.querySelector('map-visualizer-inspector') as any)?.context.chooseVisualizerCandidate).toBeTypeOf('function'));
  const inspector = f.region.element.querySelector('map-visualizer-inspector') as any;
  const choiceControl = document.createElement('button'); inspector.append(choiceControl); choiceControl.focus();
  await inspector.context.chooseVisualizerCandidate(replacement.selectedVisualizer, new AbortController().signal);
  expect(f.region.element.contains(document.activeElement)).toBe(true);
  expect((f.region.element.querySelector('map-visualizer-inspector') as any).context.visualizerInspection.selectedVisualizer).toBe(replacement.selectedVisualizer);
  expect(f.region.element.textContent).not.toContain('Select a Visualizer');
  expect(invoker.dataset.visualizerInspected).toBe('true'); f.region.dispose();
});
