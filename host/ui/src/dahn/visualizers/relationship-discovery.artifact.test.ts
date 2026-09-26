import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { NodeRelationshipDiscovery } from '../runtime/relationship-discovery';
import type { RelationshipAffordance } from '../contracts/affordances';
import { defineCustomElementOnce } from './define-custom-element-once';

const source = await readFile(resolve(process.cwd(), 'conductora/resources/dahn-visualizers/holon-inspector.js'), 'utf8');
const Node = (await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`)).default;
const relation = (label: string, plural = false) => ({ ...(plural ? { kind: 'relationship' } : {}), label, relationship: { descriptor: { relationshipName: async () => label } } }) as RelationshipAffordance;
const flush = async () => { for (let i = 0; i < 30; ++i) await Promise.resolve(); };

beforeEach(() => {
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} });
  vi.stubGlobal('requestAnimationFrame', () => 1);
  vi.stubGlobal('cancelAnimationFrame', vi.fn());
});
afterEach(() => { document.body.replaceChildren(); vi.unstubAllGlobals(); });

it('reveals population in descriptor order, shows exact counts and preserves mounted state through empty inspection', async () => {
  const parent = relation('Parent'), friends = relation('Friends', true), orders = relation('Orders', true);
  let finish!: (members: { length: number }) => void;
  const owner = { relatedHolons: vi.fn(async (name: string) => {
    if (name === 'Parent') return new Promise<{ length: number }>(resolve => { finish = resolve; });
    return { length: name === 'Friends' ? 0 : 8 };
  }) };
  const discovery = new NodeRelationshipDiscovery({} as never, owner as never, [parent, friends, orders]);
  const node = document.createElement(defineCustomElementOnce('test-discovered-node', Node)) as any;
  const properties = document.createElement('input'); properties.value = 'staged edit retained';
  const content = document.createElement('div'); content.textContent = 'existing collection';
  const activate = vi.fn((_item, _slot, publish) => publish({ state: 'loaded', content })), singular = vi.fn();
  node.setContext({ title: 'Identity', relationshipDiscovery: discovery,
    collectionActivation: { activate, dispose: () => discovery.dispose() }, activateRelationship: singular,
    nodeAffordances: { singularRelationships: [parent], collections: [friends, orders, { kind: 'property', label: 'Array' }] },
    childVisualizers: new Map([['properties', properties], ['collections', content]]) });
  document.body.append(node);
  const controls = [...node.querySelectorAll('[data-relationship-direction], [data-population]')] as HTMLButtonElement[];
  expect(node.contains(properties)).toBe(true);
  expect(node.textContent).toContain('Identity');
  expect(controls.every(control => control.style.display === 'none')).toBe(true);
  expect(owner.relatedHolons).not.toHaveBeenCalled();
  discovery.start(); await flush();
  const tabs = [...node.querySelectorAll('[role=tab]')] as HTMLButtonElement[];
  expect(tabs.map(tab => tab.textContent)).toEqual(['Friends (0)', 'Orders (8)']);
  expect(tabs[0].style.display).toBe('none'); expect(tabs[1].style.display).not.toBe('none');
  tabs[1].click(); expect(activate).toHaveBeenCalledOnce();
  const toggle = node.querySelector('[data-show-empty-relationships]') as HTMLInputElement;
  expect(toggle.checked).toBe(false);
  toggle.click(); expect(tabs[0].style.display).not.toBe('none');
  tabs[0].click(); expect(activate).toHaveBeenCalledOnce();
  expect(node.querySelector('[data-relationship-discovery-status]').textContent).toContain('Friends: No targets');
  expect(tabs[1].getAttribute('aria-selected')).toBe('true');
  toggle.click();
  finish({ length: 1 }); await flush();
  const rail = node.querySelector('[data-singular-relationship]') as HTMLButtonElement;
  expect(rail.textContent).toBe('Parent (1)'); expect(rail.style.display).not.toBe('none');
  expect(node.querySelector('input:not([type=checkbox])')).toBe(properties);
  expect(properties.value).toBe('staged edit retained'); expect(node.contains(content)).toBe(true);
  expect([...node.querySelectorAll('button')].find((button: any) => button.textContent === 'Array')).toBeDefined();
  node.remove();
});

it('provides local failure retry without presenting it as empty, including with Show Empty enabled', async () => {
  const relationship = relation('Failed', true);
  const owner = { relatedHolons: vi.fn().mockRejectedValueOnce(new Error('offline')).mockResolvedValue({ length: 0 }) };
  const discovery = new NodeRelationshipDiscovery({} as never, owner as never, [relationship]);
  const node = document.createElement(defineCustomElementOnce('test-discovery-errors', Node)) as any;
  node.setContext({ relationshipDiscovery: discovery, collectionActivation: { activate: vi.fn(), dispose: () => discovery.dispose() },
    nodeAffordances: { collections: [relationship] } });
  document.body.append(node); discovery.start(); await flush();
  node.querySelector('[data-show-empty-relationships]').click();
  const tab = node.querySelector('[role=tab]'); expect(tab.style.display).toBe('none');
  expect(tab.dataset.population).toBe('failed');
  const retry = node.querySelector('[data-relationship-discovery-status] button');
  expect(retry.textContent).toBe('Retry Failed'); retry.click(); await flush();
  expect(tab.dataset.population).toBe('empty'); expect(tab.textContent).toBe('Failed (0)');
  expect(tab.style.display).not.toBe('none');
});
