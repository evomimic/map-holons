import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';

beforeEach(() => {
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} });
  vi.stubGlobal('requestAnimationFrame', () => 1);
  vi.stubGlobal('cancelAnimationFrame', vi.fn());
});
afterEach(() => { document.body.replaceChildren(); vi.unstubAllGlobals(); });

it('packages both Node artifacts from the shared title-bar source without runtime imports', async () => {
  const root = resolve(process.cwd(), '..');
  await promisify(execFile)(process.execPath, [resolve(root, 'scripts/build-node-visualizers.mjs'), '--check'], { cwd: root });
});

it.each(['holon-inspector', 'load-holons-inspector'])('%s obeys the same title-bar interaction and compression contract', async name => {
  const source = await readFile(resolve(process.cwd(), `conductora/resources/dahn-visualizers/${name}.js`), 'utf8');
  const Node = (await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`)).default;
  customElements.define(`test-shared-title-${name}`, class extends Node {});
  const node = document.createElement(`test-shared-title-${name}`) as any;
  node.setContext({ title: 'MAP Core Schema Space', holonKey: 'MAP.CoreSchemaSpace' }); document.body.append(node);
  const bar = node.querySelector('[data-node-title-bar]') as HTMLElement;
  const title = bar.querySelector('button') as HTMLButtonElement;
  const explore = bar.querySelector('[data-explore-from-here]') as HTMLButtonElement;
  const close = bar.querySelector('[data-close-occurrence]') as HTMLButtonElement;
  const maximize = bar.querySelector('[data-maximize-inspector]') as HTMLButtonElement;
  const information = bar.querySelector('[data-visualizer-information]') as HTMLButtonElement;
  const restored = vi.fn(), explored = vi.fn(), closed = vi.fn(), inspected = vi.fn();
  node.setOccurrenceRestorationHandler(restored); node.setOccurrenceExplorationHandler(explored); node.setOccurrenceClosureHandler(closed);
  node.setOccurrenceAttentionHandler((operation: string) => { node.setOccurrenceAttentionState(operation === 'maximize'); return { status: 'applied' }; });
  node.setVisualizerInformationHandler(inspected, 'Selected Node');
  const allocation = (vertical: string, horizontal: string, width = 900, height = 760) => node.setNodeInspectorAllocation({ vertical, horizontal, width, height });
  allocation('full-height', 'full-width');
  expect(title.textContent).toBe('MAP Core Schema Space'); title.click(); expect(restored).not.toHaveBeenCalled();
  expect(explore.hidden).toBe(false); expect(explore.querySelector('svg')).not.toBeNull(); explore.click(); expect(explored).toHaveBeenCalledOnce();
  expect(close.getAttribute('aria-label')).toBe('Close branch: MAP Core Schema Space'); close.click(); expect(closed).toHaveBeenCalledOnce();
  expect(maximize.disabled).toBe(false); maximize.click(); expect(maximize.getAttribute('aria-label')).toBe('Restore Inspector');
  expect(maximize.getAttribute('aria-pressed')).toBe('true'); maximize.click(); expect(maximize.getAttribute('aria-label')).toBe('Maximize Inspector');
  information.dispatchEvent(new MouseEvent('mouseenter')); expect(bar.querySelector('[role=tooltip]')?.textContent).toBe('Selected Node');
  information.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' })); expect((bar.querySelector('[role=tooltip]') as HTMLElement).hidden).toBe(true);
  information.click(); expect(inspected).toHaveBeenCalledWith(information);
  information.dataset.visualizerInspected = 'true'; information.setAttribute('aria-pressed', 'true');
  allocation('partial-height', 'full-width'); title.click(); expect(restored).toHaveBeenCalledOnce();
  expect(title.getAttribute('aria-label')).toBe('Restore occurrence: MAP Core Schema Space');
  expect(information.getAttribute('aria-pressed')).toBe('true');
  allocation('full-height', 'minimal-width', 64); expect(title.style.writingMode).toBe('vertical-rl');
  expect(explore.hidden).toBe(true); expect(maximize.hidden).toBe(true); expect(close.hidden).toBe(false);
  allocation('minimal-height', 'minimal-width', 64, 48); expect(title.textContent).toBe('MCSS');
  expect(title.style.writingMode).toBe('horizontal-tb'); expect(information.hidden).toBe(false);
  allocation('full-height', 'full-width'); explore.focus(); node.setOccurrenceExplorationHandler(undefined);
  expect(explore.hidden).toBe(true); expect(document.activeElement).toBe(title);
  node.setVisualizerInformationHandler(undefined, 'Selected Node'); expect(information.hidden).toBe(true);
});
