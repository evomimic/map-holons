import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { afterEach, expect, it, vi } from 'vitest';

const constructors = await Promise.all(['discovery-tree', 'discovery-levels'].map(async name => {
  const source = await readFile(resolve(process.cwd(), `conductora/resources/dahn-visualizers/${name}.js`), 'utf8');
  const implementation = (await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`)).default;
  customElements.define(`test-${name}`, implementation);
  return `test-${name}`;
}));
const ref = (id: string) => ({ id, equals: (other: any) => other.id === id });
const leaf = ref('Leaf'), general = ref('General'), current = ref('Current'), alternative = ref('Alternative'), rejected = ref('Rejected');
const evidence = {
  levels: [{ descriptor: leaf, label: 'Specific type' }, { descriptor: general, label: 'General type' }],
  candidates: [
    { visualizer: current, label: 'Current view', declarations: [leaf], assessment: 'viable', current: true },
    { visualizer: alternative, label: 'General view', declarations: [general], assessment: 'viable', current: false },
    { visualizer: rejected, label: 'Unavailable view', declarations: [leaf], assessment: 'incompatible_slot', current: false },
  ], endpoint: 'General type', stopReason: 'holon_type_boundary', rationale: 'Selection rationale is unavailable.',
};
afterEach(() => document.body.replaceChildren());

it.each(constructors)('%s renders supplied declaration evidence and separates inspection from choice', async tag => {
  const element = document.createElement(tag) as any; document.body.append(element);
  const inspect = vi.fn(), choose = vi.fn(async () => {});
  const state: any = {};
  element.setContext({ discoveryExplorer: { evidence, state }, inspectVisualizerCandidate: inspect, chooseVisualizerCandidate: choose });
  await element.ready;
  expect(element.textContent).toContain('HolonType boundary: General type');
  expect(element.textContent).toContain('Selection rationale is unavailable');
  const unavailable = [...element.querySelectorAll('[data-discovery-candidate]')].find((row: any) => row.textContent.includes('Unavailable view')) as HTMLElement;
  expect([...unavailable.querySelectorAll('button')].find(button => button.textContent === 'Choose')!.disabled).toBe(true);
  const row = [...element.querySelectorAll('[data-discovery-candidate]')].find((row: any) => row.textContent.includes('General view')) as HTMLElement;
  [...row.querySelectorAll('button')].find(button => button.textContent === 'Inspect')!.click();
  expect(inspect).toHaveBeenCalledWith(alternative, expect.any(HTMLElement));
  expect(choose).not.toHaveBeenCalled(); expect(state.preview).toBe(alternative);
  element.dispose();
});

it('retains semantic candidate/preview selection between independent implementations while private expansion resets', async () => {
  const state: any = { selected: alternative, preview: alternative };
  const inspect = vi.fn(async () => {});
  for (const tag of constructors) {
    const element = document.createElement(tag) as any; document.body.append(element);
    element.setContext({ discoveryExplorer: { evidence, state }, inspectVisualizerCandidate: inspect }); await element.ready;
    expect(element.querySelector('button[aria-pressed="true"]')?.textContent).toContain('General view');
    expect(state.selected).toBe(alternative); expect(state.preview).toBe(alternative);
    expect(element.querySelectorAll('[data-discovery-level]')).toHaveLength(2);
    element.remove();
  }
  expect(inspect).toHaveBeenCalledTimes(2);
});

it('cancels an outstanding choice when its realization is disposed', async () => {
  const element = document.createElement(constructors[1]) as any; document.body.append(element);
  let signal!: AbortSignal;
  let finish!: () => void;
  element.setContext({ discoveryExplorer: { evidence, state: {} }, chooseVisualizerCandidate: (_candidate: unknown, captured: AbortSignal) => {
    signal = captured; return new Promise<void>(resolve => { finish = resolve; });
  } }); await element.ready;
  [...element.querySelectorAll('button')].find((button: any) => button.textContent === 'Choose' && !button.disabled).click();
  expect(signal.aborted).toBe(false); element.dispose(); expect(signal.aborted).toBe(true);
  finish(); await Promise.resolve(); element.dispose();
});

it('restores a retained preview without overwriting the independently selected declaration', async () => {
  const state: any = { selected: general, preview: alternative };
  const inspect = vi.fn(async () => {});
  for (const tag of constructors) {
    const element = document.createElement(tag) as any;
    element.setContext({ discoveryExplorer: { evidence, state }, inspectVisualizerCandidate: inspect });
    await element.ready; document.body.append(element);
    expect(state.selected).toBe(general); expect(state.preview).toBe(alternative);
    element.remove();
  }
  expect(inspect).toHaveBeenCalledTimes(2);
});
