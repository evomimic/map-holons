import '@angular/compiler';
import { LoadHolonsDialogService } from '../load-holons-dialog/load-holons-dialog.service';
import { Injector, runInInjectionContext } from '@angular/core';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { CanvasHostComponent } from './canvas-host.component';
import { ApplicationSessionService } from '../../services/application-session.service';
import { CanvasNavigationGuard } from '../../services/canvas-navigation-guard';

const fixture = vi.hoisted(() => {
  const reference = { key: vi.fn(async () => 'Home'), versionedKey: vi.fn(async () => 'Home@1') };
  return {
    reference,
    transaction: {
      bindPersistedReference: vi.fn(() => reference),
      getSavedHolonByBaseKey: vi.fn(async () => reference),
      commit: vi.fn(), abandon: vi.fn(), revert: vi.fn(),
    },
    begin: vi.fn(), realize: vi.fn(), realizeNode: vi.fn(), slot: vi.fn(),
    dismiss: vi.fn(), navigationDispose: vi.fn(),
  };
});
vi.mock('../../../dahn/deps/map-sdk', async importOriginal => ({
  ...await importOriginal<object>(),
  MapClient: class { beginTransaction = fixture.begin; },
}));
vi.mock('../../../dahn/runtime/materialized-visualizer-runtime', () => ({
  MaterializedVisualizerRuntime: class { realize = fixture.realize; slot = fixture.slot; },
}));
vi.mock('../../../dahn/runtime/realize-node', () => ({ realizeNode: fixture.realizeNode }));
vi.mock('../../../dahn/runtime/path-navigator', () => ({
  PathNavigator: class { canDismiss = () => true; dispose = fixture.navigationDispose; },
}));
vi.mock('../../../dahn/themes/theme', () => ({
  Theme: class { async toCssCustomProperties() { return { themeKey: 'Test', cssCustomProperties: {} }; } },
}));
vi.mock('../../../dahn/themes/offered-canvas-themes', () => ({ offeredCanvasThemes: async () => [] }));
vi.mock('../../startup-overlay', () => ({ dismissStartupOverlay: fixture.dismiss }));

let resize!: (width: number, height: number) => void;
let disconnect: ReturnType<typeof vi.fn>;
let component: CanvasHostComponent;
let display: HTMLElement;
let injector: ReturnType<typeof Injector.create>;

beforeEach(() => {
  vi.clearAllMocks();
  fixture.begin.mockResolvedValue(fixture.transaction);
  fixture.realize.mockReset().mockResolvedValue(class extends HTMLElement { setContext() {} });
  fixture.realizeNode.mockReset();
  disconnect = vi.fn();
  resize = () => { throw new Error('Observer not initialized'); };
  vi.stubGlobal('ResizeObserver', class {
    constructor(private readonly callback: ResizeObserverCallback) {}
    observe(target: Element) {
      resize = (width, height) => this.callback([{ target, contentRect: { width, height } } as ResizeObserverEntry], this as unknown as ResizeObserver);
    }
    disconnect = disconnect;
  });
  vi.spyOn(console, 'info').mockImplementation(() => {});
  vi.spyOn(console, 'table').mockImplementation(() => {});
  display = document.createElement('main');
  document.body.append(display);
});
afterEach(() => {
  component?.ngOnDestroy();
  injector?.destroy();
  document.body.replaceChildren();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

async function launch(home: boolean) {
  injector = Injector.create({ providers: [CanvasNavigationGuard, { provide: LoadHolonsDialogService, useValue: { openLoadHolons: vi.fn() } }, { provide: ApplicationSessionService, useValue: {
    waitForReady: async () => ({ phase: 'ready', active_holon_space: {},
      canvas_selection: { theme_key: 'theme', canvas_key: 'canvas', canvas_visualizer_key: 'visualizer' },
      home_dancer_selection: home ? { dancer: {}, rooted_navigation_visualizer: {}, root_node_visualizer: {} } : null,
    }),
  } }] });
  component = runInInjectionContext(injector, () => new CanvasHostComponent());
  Object.assign(component, { canvasHost: { nativeElement: display } });
  component.ngAfterViewInit();
  await vi.waitFor(() => expect(display.querySelector('[data-dahn-context]')).not.toBeNull());
}

it('mounts the selected empty Canvas only after a grant and tears it down without abandoning its transaction', async () => {
  await launch(false);
  expect(fixture.begin).toHaveBeenCalledTimes(1);
  expect(display.querySelector('[data-dahn-canvas]')).toBeNull();
  resize(480, 360);
  await vi.waitFor(() => expect(fixture.dismiss).toHaveBeenCalledTimes(1));
  const root = display.querySelector<HTMLElement>('[data-dahn-canvas]')!;
  expect(root.style.width).toBe('480px');
  expect(display.querySelector('[data-dahn-canvas-empty-state]')).not.toBeNull();
  resize(600, 400);
  expect(root.style.height).toBe('400px');
  component.ngOnDestroy();
  expect(display.children).toHaveLength(0);
  expect(disconnect).toHaveBeenCalledTimes(1);
  expect(fixture.transaction.commit).not.toHaveBeenCalled();
  expect(fixture.transaction.abandon).not.toHaveBeenCalled();
  expect(fixture.transaction.revert).not.toHaveBeenCalled();
});

it('keeps Canvas and Space Navigator available with Retry when the initial exploration fails', async () => {
  fixture.realize.mockResolvedValueOnce(class {}).mockRejectedValueOnce(new Error('Dancer unavailable'));
  await launch(true);
  resize(480, 360);
  await vi.waitFor(() => expect(fixture.dismiss).toHaveBeenCalledTimes(1));
  expect(display.querySelector('[role="alert"]')?.textContent).toContain('Dancer unavailable');
  expect(display.querySelector('[aria-busy="true"]')).toBeNull();
  expect(display.querySelector('[data-dahn-canvas-chrome]')).not.toBeNull();
  expect(display.querySelector('[role="alert"]')?.textContent).toContain('Retry');
});

it('disposes a root produced after context destruction instead of attaching it or creating navigation', async () => {
  let resolveRoot!: (root: unknown) => void;
  fixture.realizeNode.mockImplementation(() => new Promise(resolve => { resolveRoot = resolve; }));
  await launch(true);
  resize(480, 360);
  await vi.waitFor(() => expect(fixture.realizeNode).toHaveBeenCalledTimes(1));
  component.ngOnDestroy();
  const releaseRoot = vi.fn();
  resolveRoot({ element: document.createElement('div'), collectionActivation: { dispose: releaseRoot } });
  await vi.waitFor(() => expect(releaseRoot).toHaveBeenCalledTimes(1));
  expect(fixture.navigationDispose).not.toHaveBeenCalled();
  expect(display.children).toHaveLength(0);
  expect(fixture.dismiss).not.toHaveBeenCalled();
  expect(fixture.transaction.abandon).not.toHaveBeenCalled();
});

it('keeps a mounted home Dancer inside the grant and disposes navigation separately from its transaction', async () => {
  fixture.realizeNode.mockResolvedValue({ element: document.createElement('div'), collectionActivation: { dispose: vi.fn() } });
  fixture.slot.mockResolvedValue({});
  await launch(true);
  resize(480, 360);
  await vi.waitFor(() => expect(fixture.dismiss).toHaveBeenCalledTimes(1));
  expect(display.querySelector('[data-dahn-canvas-state="mounted"]')).not.toBeNull();
  expect(display.querySelector('[data-dahn-dancer-title]')?.textContent).toBe('Space Navigator');
  expect(display.querySelector('img[alt="MAP"]')?.getAttribute('src')).toBe('assets/branding/map-mark.png');
  expect(display.textContent).not.toContain('Desktop workspace');
  expect(display.textContent).not.toContain('MAP Canvas');
  expect(display.querySelector('[data-dahn-canvas-slot]')?.children).toHaveLength(1);
  component.ngOnDestroy();
  expect(fixture.navigationDispose).toHaveBeenCalledTimes(1);
  expect(display.children).toHaveLength(0);
  expect(fixture.transaction.commit).not.toHaveBeenCalled();
  expect(fixture.transaction.abandon).not.toHaveBeenCalled();
  expect(fixture.transaction.revert).not.toHaveBeenCalled();
});
