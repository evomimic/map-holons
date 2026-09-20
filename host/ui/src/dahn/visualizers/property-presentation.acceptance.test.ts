import { readFile } from 'node:fs/promises';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { defineCustomElementOnce } from './define-custom-element-once';

type Materialized = { source: string; entrypoint: string; selected: string };
type Element = HTMLElement & { setContext(context: unknown): void };
async function realize(module: Materialized): Promise<Element> {
  const exports = await import(`data:text/javascript;base64,${Buffer.from(module.source).toString('base64')}`);
  return document.createElement(defineCustomElementOnce('test-acceptance-selected', exports[module.entrypoint])) as Element;
}

// Produced by the focused Sweettest after committed descriptor discovery and
// Rust selection/materialization. No selection policy lives in this consumer.
const evidencePath = process.env['MAP_VALUE_PRESENTATION_EVIDENCE'];
describe.skipIf(evidencePath === undefined)('committed Book presentation acceptance', () => {
  afterEach(() => vi.unstubAllGlobals());
  it('renders the actual Rust-selected artifacts and every committed scalar family', async () => {
    // jsdom checks composition; the browser harness checks actual layout.
    vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} });
    const evidence = JSON.parse(await readFile(evidencePath!, 'utf8'));
    const children = new Map<string, HTMLElement>();
    for (const field of evidence.fields) {
      const value = await realize(field.visualizer);
      value.setContext({ propertyPresentation: { propertyName: field.name, value: field.value } });
      const property = await realize(field.property);
      property.setContext({ propertyPresentation: { propertyName: field.name }, childVisualizers: new Map([['value', value]]) });
      children.set(field.name, property);
    }
    const properties = await realize(evidence.properties);
    properties.setContext({ childVisualizers: children });
    const node = await realize(evidence.node);
    node.setContext({ title: 'Book scalar acceptance', childVisualizers: new Map([['properties', properties]]) });
    document.body.append(node);
    for (const [name, expected] of Object.entries({ Title: 'Book.InverseSchema.Instance', IsPublished: 'false', PageCount: '0', PublicationStatus: 'Draft', CoverDigest: '[3 bytes]', Subtitle: '' })) {
      const slot = node.querySelector(`[data-dahn-property-slot="${name}"]`)!;
      expect(slot).not.toBeNull();
      expect(slot.querySelector('[data-dahn-scalar-value]')?.textContent).toBe(expected);
    }
    const pane = node.querySelector('[data-holon-inspector-property-viewer]') as HTMLElement;
    const actions = node.querySelector('[data-holon-inspector-action-bar]') as HTMLElement;
    const collections = node.querySelector('[data-holon-inspector-collection-tab-bar]')!;
    expect(pane.style.gridColumn).toBe('1');
    expect(pane.style.gridRow).toBe('2');
    expect(actions.style.gridRow).toBe('1');
    expect(pane.compareDocumentPosition(collections) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(node.querySelector('input,textarea,select,[contenteditable]')).toBeNull();
    node.remove();
  });
});
