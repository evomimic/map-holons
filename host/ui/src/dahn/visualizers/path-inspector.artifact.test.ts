import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

type PathInspectorElement = HTMLElement & {
  setContext(context: { title?: string }): void;
};

async function loadPathInspector(): Promise<new () => PathInspectorElement> {
  const artifactPath = resolve(
    process.cwd(),
    'conductora/resources/dahn-visualizers/path-inspector.js',
  );
  const source = await readFile(artifactPath, 'utf8');
  const module = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
  return module.default as new () => PathInspectorElement;
}

describe('Path Inspector visualizer artifact', () => {
  it('renders the requested initial geometry and inspection landmarks', async () => {
    const PathInspector = await loadPathInspector();
    const tagName = 'map-path-inspector-artifact-test';
    customElements.define(tagName, PathInspector);
    const element = document.createElement(tagName) as PathInspectorElement;

    element.setContext({ title: 'Active HolonSpace' });

    expect(element.dataset.dahnPathInspector).toBe('true');
    expect(element.style.display).toBe('grid');
    expect(element.querySelector('[data-path-inspector-title]')?.textContent).toBe('Active HolonSpace');
    expect(element.querySelector('[data-path-inspector-action-bar]')).not.toBeNull();
    expect(element.querySelector('[data-path-inspector-property-viewer]')).not.toBeNull();
    expect(element.querySelector('[data-path-inspector-single-value-rail]')).not.toBeNull();
    expect(element.querySelector('[data-path-inspector-collection-tab-bar]')).not.toBeNull();
  });
});
