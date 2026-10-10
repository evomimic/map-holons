import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { afterEach, expect, it, vi } from 'vitest';
const source = await readFile(resolve(process.cwd(), 'conductora/resources/dahn-visualizers/visualizer-inspector.js'), 'utf8');
const Inspector = (await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`)).default;
customElements.define('test-semantic-visualizer-inspector', Inspector);
afterEach(() => document.body.replaceChildren());

it('shows only consumed tokens with their exact-version assignments from the current Theme', async () => {
  const element = document.createElement('test-semantic-visualizer-inspector') as any;
  document.body.append(element);
  const context = {
    theme: { themeKey: 'Ocean', tokenAssignmentValues: { 'Surface@1': '#123456', 'Surface@2': '#abcdef', 'Unused@1': '9px' } },
    holon: {
      propertyValue: async () => ({ StringValue: 'Table' }), key: async () => 'Table', availableProperties: async () => [],
      availableRelationships: async () => [{ descriptor: { relationshipName: async () => 'ConsumesDesignToken' } }],
      expandRelationship: async () => [
        { key: async () => 'Surface', versionedKey: async () => 'Surface@1' },
        { key: async () => 'Missing', versionedKey: async () => 'Missing@1' },
      ],
    },
  };
  element.setContext(context); await element.ready;
  expect([...element.querySelectorAll('[data-theme-token-value]')].map(item => item.textContent)).toEqual(['#123456', 'Assignment unavailable']);
  expect(element.textContent).toContain('Current Theme: Ocean');
  expect(element.textContent).not.toContain('Unused');
  element.setContext({ ...context, theme: { themeKey: 'Parchment', tokenAssignmentValues: { 'Surface@1': '#fedcba' } } }); await element.ready;
  expect(element.textContent).toContain('Current Theme: Parchment');
  expect(element.querySelector('[data-theme-token-value]')?.textContent).toBe('#fedcba');
  expect(element.textContent).not.toContain('#123456');
});

it('offers the standard exploration icon as an explicit definition navigation action', async () => {
  const element = document.createElement('test-semantic-visualizer-inspector') as any;
  document.body.append(element);
  const explore = vi.fn();
  element.setContext({ onExploreVisualizer: explore, holon: {
    propertyValue: async () => ({ StringValue: 'Table Collection' }), key: async () => 'Table',
    availableProperties: async () => [], availableRelationships: async () => [],
  } });
  await element.ready;
  expect(explore).not.toHaveBeenCalled();
  const control = element.querySelector('[data-explore-from-here]') as HTMLButtonElement;
  expect(control.getAttribute('aria-label')).toBe('Explore from here');
  expect(control.querySelector('svg')).not.toBeNull();
  control.click(); expect(explore).toHaveBeenCalledTimes(1);
});

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
  expect(element.querySelector('[data-visualizer-technical-details] h3')?.textContent).toBe('Definition properties');
  expect(element.querySelector('section')).not.toBeNull();
  expect([...element.querySelectorAll('details')].every((details: any) => !details.open)).toBe(true);
  const technical = element.querySelector('[data-visualizer-technical-details]') as HTMLDetailsElement;
  expect(technical.open).toBe(false);
  expect(technical.querySelector(':scope > summary')?.textContent).toBe('Technical details');
  expect(technical.contains(element.querySelector('[data-visualizer-slot-information]'))).toBe(true);
  expect(technical.textContent).toContain('occurrence-A');
  expect(technical.textContent).toContain('Book.Visualizer');
  expect([...element.children].map(child => child.tagName)).toEqual(['H2', 'SECTION', 'H3', 'P', 'DETAILS']);
  technical.open = true;
  expect(technical.querySelector('section dd')?.textContent).toBe('true');
});

it('shows the actual slot purpose separately from the selected visualizer approach without discovering candidates', async () => {
  const element = document.createElement('test-semantic-visualizer-inspector') as any;
  document.body.append(element);
  const slotRead = vi.fn(async (name: string) => name === 'VisualizerSlotDescription' ? { StringValue: 'Inspect an item and its connections.' } : null);
  const definitionRead = vi.fn(async (name: string) => ({ StringValue: name === 'DisplayName' ? 'Holon Inspector' : 'Uses a titled card with property rows and collection tabs.' }));
  element.setContext({ holon: { key: async () => 'HolonInspector', propertyValue: definitionRead,
    availableProperties: async () => [], availableRelationships: async () => [],
  }, visualizerInspection: { occurrenceId: 'node', slot: { propertyValue: slotRead, key: async () => 'RootNodeSlot', relatedHolons: async () => [] },
    subject: { key: async () => 'Item' },
  } });
  await element.ready;
  const purpose = element.querySelector('[data-visualizer-slot-purpose]');
  const approach = element.querySelector('[data-visualizer-description]');
  const technical = element.querySelector('[data-visualizer-technical-details]');
  expect(purpose.textContent).toContain('Inspect an item and its connections.');
  expect(approach.textContent).toBe('Uses a titled card with property rows and collection tabs.');
  expect(technical.contains(purpose)).toBe(false);
  expect(technical.contains(approach)).toBe(false);
  expect(technical.open).toBe(false);
  expect(slotRead).toHaveBeenCalledWith('VisualizerSlotDescription');
  expect(definitionRead).toHaveBeenCalledWith('VisualizerDescription');
  expect(element.textContent).not.toContain('also available');
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

it('expands child information inline while preserving parent context and other open entries', async () => {
  const element = document.createElement('test-semantic-visualizer-inspector') as any;
  const source = document.createElement('div'), child = document.createElement('div'); source.append(child);
  document.body.append(source, element);
  const selected = { occurrenceId: 'child', isLive: () => child.isConnected };
  const owned = { occurrenceId: 'rail', isLive: () => source.isConnected };
  let entries = [
    { label: 'Properties', ownership: 'selected', displayName: 'Property Map', inspect: () => child.isConnected ? selected : undefined },
    { label: 'Vertical Rail', ownership: 'implementation', inspect: () => owned },
  ];
  const inspect = vi.fn();
  const mount = vi.fn(async (target, host) => { host.textContent = `Details for ${target.occurrenceId}`; });
  element.setContext({ holon: { key: async () => 'Node', propertyValue: async (name: string) => ({ StringValue: name === 'DisplayName' ? 'Holon Inspector' : 'Inspect an item.' }),
    availableProperties: async () => [], availableRelationships: async () => [],
  }, onInspectVisualizer: inspect, mountVisualizerInformation: mount, visualizerInspection: { occurrenceId: 'parent', element: source, isLive: () => source.isConnected,
    slot: { propertyValue: async () => ({ StringValue: 'Node' }), relatedHolons: async () => [] }, subject: { key: async () => 'Item' },
    composition: () => entries,
  } });
  await element.ready;
  const disclosure = element.querySelector('[data-visualizer-presentation-structure]') as HTMLDetailsElement;
  expect(disclosure.open).toBe(false);
  expect(disclosure.textContent).toContain('Properties · Property Map');
  expect(disclosure.textContent).toContain('Vertical Rail · Bundled Component');
  expect(element.querySelector('[data-visualizer-technical-details]').contains(disclosure)).toBe(false);
  const panels = [...disclosure.querySelectorAll<HTMLDetailsElement>('[data-visualizer-child-information]')];
  expect(mount).not.toHaveBeenCalled();
  panels[0].open = true;
  await vi.waitFor(() => expect(panels[0].textContent).toContain('Details for child'));
  expect(mount).toHaveBeenLastCalledWith(selected, expect.any(HTMLElement));
  expect(element.querySelector('h2')?.textContent).toBe('Holon Inspector');
  panels[0].open = false;
  await new Promise(resolve => setTimeout(resolve, 0));
  panels[0].open = true;
  await new Promise(resolve => setTimeout(resolve, 0));
  expect(mount).toHaveBeenCalledTimes(1);
  panels[1].open = true;
  await vi.waitFor(() => expect(panels[1].textContent).toContain('Included in Holon Inspector.'));
  expect(panels[1].textContent).toContain('This component has no independent VisualizerSlot and cannot be selected separately.');
  expect(panels[1].textContent).not.toContain('Details for rail');
  expect(panels[1].querySelector('h2')).toBeNull();
  expect(mount).toHaveBeenCalledTimes(1);
  expect(inspect).not.toHaveBeenCalled();
  entries = entries.slice(1); child.remove();
  await vi.waitFor(() => expect(disclosure.querySelectorAll('[data-visualizer-child-information]')).toHaveLength(1));
  expect(panels[1].isConnected).toBe(true);
  expect(panels[1].open).toBe(true);
  expect(panels[1].textContent).toContain('Included in Holon Inspector.');
  element.remove(); source.append(document.createElement('span'));
  await Promise.resolve();
  expect(disclosure.querySelectorAll('[data-visualizer-child-information]')).toHaveLength(1);
});

it('keeps selected children inside bundled components inspectable and removes retired children', async () => {
  const element = document.createElement('test-semantic-visualizer-inspector') as any;
  const source = document.createElement('div'), rail = document.createElement('aside'), child = document.createElement('div');
  rail.append(child); source.append(rail); document.body.append(source, element);
  const selected = { occurrenceId: 'connection', isLive: () => child.isConnected };
  const owned = { occurrenceId: 'rail', element: rail, isLive: () => rail.isConnected,
    composition: () => child.isConnected ? [{ label: 'Connection', ownership: 'selected', displayName: 'Book Inspector', inspect: () => selected }] : [],
  };
  const mount = vi.fn(async (target, host) => { host.textContent = `Details for ${target.occurrenceId}`; });
  element.setContext({ holon: {
    key: async () => 'Node', propertyValue: async (name: string) => ({ StringValue: name === 'DisplayName' ? 'Holon Inspector' : 'Inspect an item.' }),
    availableProperties: async () => [], availableRelationships: async () => [],
  }, mountVisualizerInformation: mount, visualizerInspection: {
    occurrenceId: 'parent', element: source, isLive: () => source.isConnected,
    slot: { propertyValue: async () => ({ StringValue: 'Node' }), relatedHolons: async () => [] }, subject: { key: async () => 'Item' },
    composition: () => rail.isConnected ? [{ label: 'Vertical Rail', ownership: 'implementation', inspect: () => owned }] : [],
  } });
  await element.ready;
  const bundled = element.querySelector('[data-visualizer-child-information]') as HTMLDetailsElement;
  bundled.open = true;
  await vi.waitFor(() => expect(bundled.textContent).toContain('Connection · Book Inspector'));
  expect(mount).not.toHaveBeenCalled();
  expect(bundled.querySelector('[data-visualizer-slot-purpose]')).toBeNull();
  expect(bundled.querySelector('[data-visualizer-description]')).toBeNull();
  const nested = bundled.querySelector('[data-visualizer-child-information]') as HTMLDetailsElement;
  nested.open = true;
  await vi.waitFor(() => expect(nested.textContent).toContain('Details for connection'));
  expect(mount).toHaveBeenCalledExactlyOnceWith(selected, expect.any(HTMLElement));
  child.remove();
  await vi.waitFor(() => expect(bundled.querySelector('[data-visualizer-child-information]')).toBeNull());
  expect(bundled.isConnected).toBe(true);
  expect(bundled.open).toBe(true);
  rail.remove();
  await vi.waitFor(() => expect(element.querySelector('[data-visualizer-child-information]')).toBeNull());
});
