import { beforeEach, expect, it, vi } from 'vitest';
import { allocateInitialWindow } from './initial-window-allocation';

const native = vi.hoisted(() => ({
  isTauri: vi.fn(() => true), setSize: vi.fn(),
  innerSize: vi.fn(async () => ({ width: 1280, height: 1040, toLogical: () => ({ width: 1280, height: 1040 }) })),
  outerSize: vi.fn(async () => ({ width: 1280, height: 1068 })),
  scaleFactor: vi.fn(async () => 1),
  currentMonitor: vi.fn(async () => ({ workArea: { size: { toLogical: () => ({ width: 1920, height: 1440 }) } } })),
}));
vi.mock('@tauri-apps/api/core', () => ({ isTauri: native.isTauri }));
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => native, currentMonitor: native.currentMonitor,
  LogicalSize: class { constructor(public width: number, public height: number) {} },
}));
beforeEach(() => { vi.clearAllMocks(); native.isTauri.mockReturnValue(true); });
it('grows the initial window to the composed report without shrinking existing width', async () => {
  await allocateInitialWindow({ minimum: { width: 1240, height: 1300 } });
  expect(native.setSize).toHaveBeenCalledWith(expect.objectContaining({ width: 1280, height: 1300 }));
});
it('respects the monitor work area and native decorations when composition cannot fit', async () => {
  await allocateInitialWindow({ minimum: { width: 2400, height: 2000 } });
  expect(native.setSize).toHaveBeenCalledWith(expect.objectContaining({ width: 1920, height: 1412 }));
});
it('leaves browser embedding under its external allocation owner', async () => {
  native.isTauri.mockReturnValue(false);
  await allocateInitialWindow({ minimum: { width: 2400, height: 2000 } });
  expect(native.innerSize).not.toHaveBeenCalled();
  expect(native.setSize).not.toHaveBeenCalled();
});
