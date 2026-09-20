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
  const title = element.querySelector('h2')!;
  vi.spyOn(title, 'getBoundingClientRect').mockImplementation(() => rect(20));
  const footer = element.querySelector('footer')!;
  vi.spyOn(footer, 'getBoundingClientRect').mockImplementation(() => rect(30));
  const rows = [...element.querySelectorAll<HTMLElement>('[data-dahn-property-slot]')];
  rows.forEach((row, index) => vi.spyOn(row, 'getBoundingClientRect').mockImplementation(() => rect(heights[index])));
  element.querySelector<HTMLElement>('[data-dahn-properties-rows]')!.style.gap = '5px';
  document.body.append(element); flush();
  return {
    element, children, rows, footer,
    button: element.querySelector<HTMLButtonElement>('button')!,
    list: element.querySelector<HTMLElement>('[data-dahn-properties-list]')!,
    resize(next: number) { height = next; resized(); flush(); },
  };
}
const visible = (rows: HTMLElement[]) => rows.filter(row => row.style.visibility === 'visible');

describe('bounded Properties disclosure', () => {
  it('shows an exact-fitting ordered prefix and reserves the footer', async () => {
    const f = await fixture([40, 50, 60], 153); // 153 - heading/gap 24 - footer/gap 34 = 95
    expect(visible(f.rows)).toEqual(f.rows.slice(0, 2));
    expect(f.button.textContent).toContain('Show 1 more property');
    expect(f.rows[2].inert).toBe(true);
    expect(f.rows[2].getAttribute('aria-hidden')).toBe('true');
    expect(f.rows[2].firstElementChild).toBe(f.children[2]);
    expect(f.list.style.overflowY).toBe('hidden');
    expect(f.button.getAttribute('aria-controls')).toBe(f.list.id);
  });

  it('shows all rows and omits the footer when everything fits without it', async () => {
    const f = await fixture([40, 50, 60], 184);
    expect(visible(f.rows)).toEqual(f.rows);
    expect(f.footer.style.position).toBe('absolute');
    expect(f.footer.inert).toBe(true);
  });

  it('expands only the list, retains focus, and collapses back to the prefix', async () => {
    const f = await fixture([40, 50, 60], 153);
    f.button.focus(); f.button.click(); flush();
    expect(visible(f.rows)).toEqual(f.rows);
    expect(f.list.style.overflowY).toBe('auto');
    expect(f.list.tabIndex).toBe(0);
    expect(f.button.getAttribute('aria-expanded')).toBe('true');
    expect(f.button.textContent).toContain('Show fewer properties');
    expect(document.activeElement).toBe(f.button);
    f.list.scrollTop = 60; f.button.click(); flush();
    expect(f.list.scrollTop).toBe(0);
    expect(visible(f.rows)).toEqual(f.rows.slice(0, 2));
    expect(document.activeElement).toBe(f.button);
  });

  it('recomputes after allocation and row-height changes without reordering', async () => {
    const heights = [40, 50, 60];
    const f = await fixture(heights, 153);
    heights[0] = 90; resized(); flush();
    expect(visible(f.rows)).toEqual(f.rows.slice(0, 1));
    f.button.focus();
    f.resize(300);
    expect(document.activeElement).toBe(f.element.querySelector('h2'));
    expect(visible(f.rows)).toEqual(f.rows);
    f.resize(80);
    expect(visible(f.rows)).toHaveLength(0);
    expect(f.button.textContent).toContain('Show 3 more properties');
    f.resize(40);
    expect(f.element.querySelector('h2')!.style.visibility).toBe('hidden');
    expect(f.footer.inert).toBe(false);
  });

  it('handles empty sets and disconnects observation', async () => {
    const f = await fixture([], 100);
    expect(f.list.textContent).toBe('No properties');
    expect(f.footer.inert).toBe(true);
    f.element.remove();
    expect(disconnect).toHaveBeenCalled();
  });
});
