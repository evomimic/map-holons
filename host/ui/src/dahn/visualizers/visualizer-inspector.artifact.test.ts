import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { afterEach, expect, it, vi } from 'vitest';
const source = await readFile(resolve(process.cwd(), 'conductora/resources/dahn-visualizers/visualizer-inspector.js'), 'utf8');
const Inspector = (await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`)).default;
customElements.define('test-semantic-visualizer-inspector', Inspector);
afterEach(() => document.body.replaceChildren());

it('presents the shared definition and semantic relationships separately from the occurrence subject', async () => {
  const element = document.createElement('test-semantic-visualizer-inspector') as any;
  document.body.append(element);
  const read = vi.fn(async (name: string) => name === 'DisplayName' ? { StringValue: 'Book Inspector' } : name === 'Enabled' ? { BooleanValue: true } : null);
  element.setContext({ holon: { key: async () => 'Book.Visualizer', propertyValue: read,
    availableRelationships: async () => ['ApplicableToType', 'HasSlot', 'ImplementedBy'].map(name => ({ descriptor: { relationshipName: async () => name } })),
    expandRelationship: async (name: string) => [{ key: async () => ({ ApplicableToType: 'Book.HolonType', HasSlot: 'Book.PropertiesSlot', ImplementedBy: 'Book.TypeScript' })[name] }],
    availableProperties: async () => [{ propertyName: async () => 'Enabled', displayName: async () => 'Enabled' }],
  }, visualizerInspection: { occurrenceId: 'occurrence-A', slot: {
    propertyValue: async () => ({ StringValue: 'Book details' }), key: async () => 'Book.NodeSlot',
    relatedHolons: async (name: string) => name === 'AcceptsVisualizerType' ? [{ propertyValue: async () => ({ StringValue: 'Node Visualizer' }), key: async () => 'NodeVisualizer.HolonType' }] : [],
  }, subject: { key: async () => 'Book-123' } } });
  await element.ready;
  expect(element.querySelector('h2')?.textContent).toBe('Book Inspector');
  expect(element.textContent).toContain('A description is not available for this view yet.');
  expect(element.textContent).toContain('Presenting: Book-123 · occurrence-A');
  expect(element.textContent).toContain('Applicable typesBook.HolonType');
  expect(element.textContent).toContain('Composition slotsBook.PropertiesSlot');
  expect(element.textContent).toContain('ImplementationsBook.TypeScript');
  expect(element.querySelector('section dd')?.textContent).toBe('true');
  expect([...element.querySelectorAll('[data-visualizer-slot-information] dt')].map(term => term.textContent)).toEqual(['Visualizer Slot Kind', 'Slot name']);
  expect([...element.querySelectorAll('[data-visualizer-slot-information] dd')].map(value => value.textContent)).toEqual(['Node Visualizer', 'Book details']);
  expect(element.querySelector('h3')?.textContent).toBe('Definition properties');
  expect(element.querySelector('section')).not.toBeNull();
  expect([...element.querySelectorAll('details')].every((details: any) => !details.open)).toBe(true);
  const technical = element.querySelector('[data-visualizer-technical-details]') as HTMLDetailsElement;
  expect(technical.open).toBe(false);
  expect(technical.querySelector(':scope > summary')?.textContent).toBe('Technical details');
  expect(technical.contains(element.querySelector('[data-visualizer-slot-information]'))).toBe(true);
  expect(technical.textContent).toContain('occurrence-A');
  expect(technical.textContent).toContain('Book.Visualizer');
  expect([...element.children].map(child => child.tagName)).toEqual(['H2', 'P', 'DETAILS']);
  technical.open = true;
  expect(technical.querySelector('section dd')?.textContent).toBe('true');
});

it('does not update a dismissed presentation after a late definition read', async () => {
  const element = document.createElement('test-semantic-visualizer-inspector') as any;
  document.body.append(element);
  let resolveName!: (value: unknown) => void;
  element.setContext({ holon: { propertyValue: () => new Promise(resolve => { resolveName = resolve; }), key: async () => 'Definition' } });
  element.remove(); resolveName({ StringValue: 'Late name' });
  // The old read is intentionally not awaited; removal revokes its presentation context.
  await Promise.resolve();
  expect(element.querySelector('h2')?.textContent).toBe('Visualizer');
});
