import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { MaterializedVisualizerRuntime } from './materialized-visualizer-runtime';
import { defineCustomElementOnce } from '../visualizers/define-custom-element-once';

const schema = JSON.parse(await readFile(resolve(process.cwd(), '../generated/json-imports/dahn/schema.json'), 'utf8'));
const operator = 'PresentActions.VisualizerOperator';
const targets = (key: string, name: string): string[] => schema.holons.find((item: any) => item.key === key)
  ?.relationships?.find((item: any) => item.name === name)?.target?.map((item: any) => item.$ref) ?? [];
const ref = (key: string): any => ({ key: async () => key, relatedHolons: async (name: string) => targets(key, name).map(ref) });
const importArtifact = async (name: string) => {
  const source = await readFile(resolve(process.cwd(), `conductora/resources/dahn-visualizers/${name}.js`), 'utf8');
  return (await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`)).default;
};
beforeEach(() => { vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} }); vi.stubGlobal('requestAnimationFrame', () => 1); vi.stubGlobal('cancelAnimationFrame', vi.fn()); });
afterEach(() => { document.body.replaceChildren(); vi.unstubAllGlobals(); });

it('declares one source-independent Node action slot with no mandatory Action children', () => {
  for (const slot of ['HolonInspector.ActionsSlot', 'LoadHolonsResult.ActionsSlot']) {
    expect(targets(slot, 'RequiresOperator')).toEqual([operator]);
    expect(targets(slot, 'UsesNodeActionsContract')).toEqual(['NodeActions.Contract']);
  }
  expect(targets('GenericActions.ActionBarVisualizer', 'OffersOperator')).toEqual([operator]);
  expect(targets('GenericActions.ActionBarVisualizer', 'HasSlot')).toEqual([]);
  expect(targets('NodeTitleBar.TitleBarVisualizer', 'HasSlot')).toEqual([]);
  expect(schema.holons.some((item: any) => item.key === 'NodeTitleActions.ActionBarVisualizer')).toBe(false);
});

it('admits the real common ActionBar and rejects a TitleBar lacking PresentActions', async () => {
  const runtime = new MaterializedVisualizerRuntime({} as never);
  const Bar = await importArtifact('actions'), Title = await importArtifact('node-title-bar');
  const bar = document.createElement(defineCustomElementOnce('test-common-action-bar', Bar));
  const title = document.createElement(defineCustomElementOnce('test-independent-title', Title));
  await expect(runtime.admit(bar, ref('HolonInspector.ActionsSlot'))).resolves.toBeUndefined();
  await expect(runtime.admit(title, ref('HolonInspector.ActionsSlot'))).rejects.toThrow(operator);
});

it('presents mixed bound actions as components, preserving controls and authority across updates', async () => {
  const Bar = await importArtifact('actions');
  const bar = document.createElement(defineCustomElementOnce('test-mixed-actions', Bar)) as any;
  const occurrence = vi.fn(), subject = vi.fn();
  const actions = [
    { id: 'close', kind: 'action', label: 'Close', icon: '×', available: true, activate: occurrence },
    { id: 'load', kind: 'action', label: 'Load Holons', available: true, activate: subject },
  ];
  bar.visualizerOperations[operator]({ actions }); document.body.append(bar);
  const close = bar.querySelector('[data-action-id="close"]'), load = bar.querySelector('[data-action-id="load"]');
  expect(occurrence).not.toHaveBeenCalled(); expect(subject).not.toHaveBeenCalled();
  close.click(); expect(occurrence).toHaveBeenCalledWith(close); expect(subject).not.toHaveBeenCalled();
  bar.visualizerOperations[operator]({ actions: [actions[0], { ...actions[1], available: false, reason: 'Load in progress' }] });
  expect(bar.querySelector('[data-action-id="load"]')).toBe(load); expect(load.disabled).toBe(true);
  expect(load.title).toContain('Load in progress'); load.click(); expect(subject).not.toHaveBeenCalled();
  expect(bar.getVisualizerComposition()).toEqual([]);
  expect(() => bar.visualizerOperations[operator]({ actions: [...actions, actions[0]] })).toThrow('duplicate');
  expect(bar.querySelector('[data-action-id="close"]')).toBe(close);
});

it('shows refusal feedback within the grant and preserves inspection chrome through membership changes', async () => {
  const Bar = await importArtifact('actions');
  const bar = document.createElement(defineCustomElementOnce('test-action-refusal', Bar)) as any;
  const action = { id: 'close', kind: 'action', label: 'Close', available: true, activate: () => ({ status: 'refused', reason: 'Work is still running.' }) };
  bar.visualizerOperations[operator]({ actions: [action] }); document.body.append(bar);
  const chrome = document.createElement('aside'); bar.append(chrome);
  const close = bar.querySelector('[data-action-id="close"]'); close.click();
  const status = bar.querySelector('[role="status"]');
  expect(status.textContent).toBe('Work is still running.'); expect(status.parentElement.hidden).toBe(false);
  const dismiss = [...bar.querySelectorAll('button')].find((button: any) => button.textContent === 'Dismiss') as HTMLButtonElement;
  expect(document.activeElement).toBe(dismiss); dismiss.click(); expect(document.activeElement).toBe(close);
  bar.visualizerOperations[operator]({ actions: [action, { ...action, id: 'other', label: 'Other' }] });
  expect(chrome.parentElement).toBe(bar); expect(bar.querySelector('[data-action-id="close"]')).toBe(close);
});
