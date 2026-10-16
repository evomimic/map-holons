import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { expect, it, vi } from 'vitest';
import { MaterializedVisualizerRuntime } from './materialized-visualizer-runtime';
import { defineCustomElementOnce } from '../visualizers/define-custom-element-once';

const schema = JSON.parse(await readFile(resolve(process.cwd(), '../generated/json-imports/dahn/schema.json'), 'utf8'));
const operator = 'ApplySubjectActions.VisualizerOperator';
const targets = (key: string, name: string): string[] => schema.holons.find((item: any) => item.key === key)
  ?.relationships?.find((item: any) => item.name === name)?.target?.map((item: any) => item.$ref) ?? [];
const ref = (key: string): any => ({ key: async () => key, relatedHolons: async (name: string) => targets(key, name).map(ref) });
const importArtifact = async (name: string) => {
  const source = await readFile(resolve(process.cwd(), `conductora/resources/dahn-visualizers/${name}.js`), 'utf8');
  return (await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`)).default;
};

it('distinguishes subject action slots from title action slots through required/offered contracts', () => {
  for (const slot of ['HolonInspector.ActionsSlot', 'LoadHolonsResult.ActionsSlot']) {
    expect(targets(slot, 'RequiresOperator')).toContain(operator);
  }
  expect(targets('GenericActions.ActionBarVisualizer', 'OffersOperator')).toContain(operator);
  expect(targets('NodeTitleActions.ActionBarVisualizer', 'OffersOperator')).not.toContain(operator);
  expect(targets('NodeTitleBar.ActionsSlot', 'RequiresOperator')).toEqual(['ApplyTitleActions.VisualizerOperator']);
});

it('admits the real subject bar and rejects the title bar at a subject-action slot', async () => {
  const runtime = new MaterializedVisualizerRuntime({} as never);
  const Subject = await importArtifact('actions');
  const Title = await importArtifact('title-actions');
  const subject = document.createElement(defineCustomElementOnce('test-subject-action-bar', Subject));
  const title = document.createElement(defineCustomElementOnce('test-title-action-bar', Title));
  await expect(runtime.admit(subject, ref('HolonInspector.ActionsSlot'))).resolves.toBeUndefined();
  await expect(runtime.admit(title, ref('HolonInspector.ActionsSlot'))).rejects.toThrow(operator);
});

it('applies selected subject actions without executing or replacing their bound controls', async () => {
  const Subject = await importArtifact('actions');
  const bar = document.createElement(defineCustomElementOnce('test-bound-subject-actions', Subject)) as any;
  const invoke = vi.fn();
  const child = document.createElement('button'); child.textContent = 'Load Holons'; child.onclick = invoke;
  const context = { actions: [{ id: 'load', kind: 'action', label: 'Load Holons' }], childVisualizers: new Map([['load', child]]) };
  expect(bar.visualizerOperations[operator](context)).toEqual({ status: 'applied' });
  expect(bar.querySelector('[data-action-id="load"]')).toBe(child);
  expect(invoke).not.toHaveBeenCalled(); child.click(); expect(invoke).toHaveBeenCalledOnce();
  bar.visualizerOperations[operator](context);
  expect(bar.querySelector('[data-action-id="load"]')).toBe(child);
  expect(invoke).toHaveBeenCalledOnce();
  bar.disconnectedCallback();
});
