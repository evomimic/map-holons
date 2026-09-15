import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

type PathInspectorElement = HTMLElement & {
  setContext(context: { title?: string; childVisualizers?: ReadonlyMap<string, HTMLElement> }): void;
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
  it('allocates a bounded root Node region owned by the Path Inspector', async () => {
    const PathInspector = await loadPathInspector();
    const tagName = 'map-path-inspector-artifact-test';
    customElements.define(tagName, PathInspector);
    const element = document.createElement(tagName) as PathInspectorElement;

    const rootNode = document.createElement('section');
    rootNode.dataset.rootNodeFixture = 'true';
    element.setContext({
      title: 'Active HolonSpace',
      childVisualizers: new Map([['root-node', rootNode]]),
    });

    expect(element.dataset.dahnPathInspector).toBe('true');
    expect(element.style.display).toBe('grid');
    expect(element.querySelector('[data-path-inspector-title]')?.textContent).toBe('Active HolonSpace');
    const region = element.querySelector('[data-path-inspector-root-node]');
    expect(region).not.toBeNull();
    expect(region?.querySelector('[data-root-node-fixture]')).not.toBeNull();
  });
});
