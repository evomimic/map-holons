import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { defineCustomElementOnce } from './define-custom-element-once';

type PropertiesElement = HTMLElement & { setContext(context: unknown): void };
let callbacks: FrameRequestCallback[];
let resized: () => void;
const disconnect = vi.fn();
beforeEach(() => {
  callbacks = [];
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => { callbacks.push(callback); return callbacks.length; });
  vi.stubGlobal('cancelAnimationFrame', vi.fn());
  vi.stubGlobal('ResizeObserver', class {
    constructor(callback: () => void) { resized = callback; }
    observe() {}
    disconnect = disconnect;
  });
});
afterEach(() => { document.body.replaceChildren(); vi.unstubAllGlobals(); vi.restoreAllMocks(); });
const flush = () => { const pending = callbacks.splice(0); pending.forEach(callback => callback(0)); };
const rect = (height: number) => ({ height } as DOMRect);
async function fixture(heights: number[], initialHeight: number) {
  const source = await readFile(resolve(process.cwd(), 'conductora/resources/dahn-visualizers/properties.js'), 'utf8');
  const module = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
  const element = document.createElement(defineCustomElementOnce('test-properties-disclosure', module.default)) as PropertiesElement;
  const children = heights.map((_, index) => {
    const child = document.createElement('span'); child.textContent = `Value ${index}`; return child;
  });
  element.setContext({ childVisualizers: new Map(children.map((child, index) => [`Field ${index}`, child])) });
  let height = initialHeight;
  Object.defineProperty(element, 'clientHeight', { get: () => height });
  element.style.gap = '4px';
  const rows = [...element.querySelectorAll<HTMLElement>('[data-dahn-property-slot]')];
  rows.forEach((row, index) => {
    Object.defineProperty(row, 'offsetHeight', { get: () => heights[index] });
    vi.spyOn(row, 'getBoundingClientRect').mockImplementation(() => rect(heights[index] / 2));
  });
  element.querySelector<HTMLElement>('[data-dahn-properties-rows]')!.style.gap = '5px';
  document.body.append(element); flush();
  return {
    element, children, rows,
    button: element.querySelector<HTMLButtonElement>('button')!,
    list: element.querySelector<HTMLElement>('[data-dahn-properties-list]')!,
    resize(next: number) { height = next; resized(); flush(); },
  };
}
describe('scrollable Properties', () => {
  it('keeps every property available without a disclosure at small allocations', async () => {
    const f = await fixture([40, 50, 60], 80);
    expect(f.element.querySelector('footer')).toBeNull();
    expect(f.element.querySelector('[data-properties-disclosure]')).toBeNull();
    expect(f.list.style.overflow).toBe('auto');
    expect(f.list.tabIndex).toBe(0);
    for (const [index, row] of f.rows.entries()) {
      expect(row.inert).not.toBe(true);
      expect(row.getAttribute('aria-hidden')).toBeNull();
      expect(row.firstElementChild).toBe(f.children[index]);
    }
    f.list.scrollTop = 60;
    f.resize(100);
    expect(f.list.scrollTop).toBe(60);
    expect(f.rows).toHaveLength(3);
  });
  it('handles empty sets and disconnects observation', async () => {
    const f = await fixture([], 100);
    expect(f.list.textContent).toBe('No properties');
    f.element.remove();
    expect(disconnect).toHaveBeenCalled();
  });
});
