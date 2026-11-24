import { beforeEach, expect, it, vi } from 'vitest';
import { allocateInitialWindow } from './initial-window-allocation';

const native = vi.hoisted(() => ({
  isTauri: vi.fn(() => true), setSize: vi.fn(), setPosition: vi.fn(),
  innerSize: vi.fn(async () => ({ width: 1280, height: 1040, toLogical: () => ({ width: 1280, height: 1040 }) })),
  outerSize: vi.fn(async () => ({ width: 1280, height: 1068 })),
  scaleFactor: vi.fn(async () => 1),
  currentMonitor: vi.fn(async (): Promise<any> => ({ workArea: { position: { x: 0, y: 0 }, size: { width: 1920, height: 1440, toLogical: () => ({ width: 1920, height: 1440 }) } } })),
}));
vi.mock('@tauri-apps/api/core', () => ({ isTauri: native.isTauri }));
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => native, currentMonitor: native.currentMonitor,
  LogicalSize: class { constructor(public width: number, public height: number) {} },
  PhysicalPosition: class { constructor(public x: number, public y: number) {} },
}));
beforeEach(() => { vi.clearAllMocks(); native.isTauri.mockReturnValue(true); });
it('grows the composed window and repositions it so the second startup resize cannot push its bottom offscreen', async () => {
  await allocateInitialWindow({ minimum: { width: 1240, height: 1300 } });
  expect(native.setSize).toHaveBeenCalledWith(expect.objectContaining({ width: 1280, height: 1300 }));
  expect(native.setPosition).toHaveBeenCalledWith(expect.objectContaining({ x: 320, y: 56 }));
  const position = native.setPosition.mock.calls[0][0];
  expect(position.y + 1300 + 28).toBeLessThanOrEqual(1440);
});
it('reserves desktop margins and native decorations when the composition cannot fit', async () => {
  await allocateInitialWindow({ minimum: { width: 2400, height: 2000 } });
  expect(native.setSize).toHaveBeenCalledWith(expect.objectContaining({ width: 1888, height: 1380 }));
  expect(native.setPosition).toHaveBeenCalledWith(expect.objectContaining({ x: 16, y: 16 }));
});
it('uses the work-area origin and physical scaling on a Retina display left of the primary display', async () => {
  native.scaleFactor.mockResolvedValueOnce(2);
  native.innerSize.mockResolvedValueOnce({ width: 2560, height: 2080, toLogical: () => ({ width: 1280, height: 1040 }) });
  native.outerSize.mockResolvedValueOnce({ width: 2560, height: 2136 });
  native.currentMonitor.mockResolvedValueOnce({ workArea: { position: { x: -3840, y: 50 }, size: { width: 3840, height: 2880, toLogical: () => ({ width: 1920, height: 1440 }) } } });
  await allocateInitialWindow({ minimum: { width: 2400, height: 2000 } });
  expect(native.setSize).toHaveBeenCalledWith(expect.objectContaining({ width: 1888, height: 1380 }));
  expect(native.setPosition).toHaveBeenCalledWith(expect.objectContaining({ x: -3808, y: 82 }));
});
it('retains the native allocation if the monitor cannot be determined', async () => {
  native.currentMonitor.mockResolvedValueOnce(null);
  await allocateInitialWindow({ minimum: { width: 2400, height: 2000 } });
  expect(native.setSize).not.toHaveBeenCalled();
  expect(native.setPosition).not.toHaveBeenCalled();
});
it('leaves browser embedding under its external allocation owner', async () => {
  native.isTauri.mockReturnValue(false);
  await allocateInitialWindow({ minimum: { width: 2400, height: 2000 } });
  expect(native.innerSize).not.toHaveBeenCalled();
  expect(native.setSize).not.toHaveBeenCalled();
  expect(native.setPosition).not.toHaveBeenCalled();
});
