import html from '../../../public/welcome.html?raw';
import script from '../../../public/welcome.js?raw';
import { afterEach, describe, expect, it, vi } from 'vitest';

async function start(invoke: ReturnType<typeof vi.fn>) {
  document.documentElement.innerHTML = html;
  Object.assign(window, { __TAURI__: { core: { invoke } } });
  // Run the shipped page script, including its event handler and polling.
  new Function(script)();
  await Promise.resolve();
  await Promise.resolve();
}

afterEach(() => {
  vi.useRealTimers();
  delete (window as unknown as Record<string, unknown>).__TAURI__;
});

describe('packaged Welcome', () => {
  it('follows backend startup progress while keeping constitution disabled', async () => {
    vi.useFakeTimers();
    const invoke = vi.fn()
      .mockResolvedValueOnce({ phase: 'provisioning', constitution_available: false, message: 'Preparing your space.' })
      .mockResolvedValueOnce({ phase: 'foundation-present', constitution_available: false, message: 'Foundation present; personal setup unavailable.' });
    await start(invoke);
    expect(document.getElementById('status')?.textContent).toBe('Preparing your space.');
    expect((document.getElementById('constitute') as HTMLButtonElement).disabled).toBe(true);
    await vi.advanceTimersByTimeAsync(1000);
    expect(document.getElementById('status')?.textContent).toBe('Foundation present; personal setup unavailable.');
    expect(invoke.mock.calls).toEqual([['welcome_status'], ['welcome_status']]);
    expect(vi.getTimerCount()).toBe(0);
  });

  it('does not offer constitution when a sovereign association is already recorded', async () => {
    await start(vi.fn().mockResolvedValue({ phase: 'association-present', constitution_available: false, message: 'Authorized reopening unavailable.' }));
    const button = document.getElementById('constitute') as HTMLButtonElement;
    expect(button.hidden).toBe(true);
    expect(button.disabled).toBe(true);
    expect(document.querySelector('h1')?.textContent).toBe('Welcome back to MAP');
  });

  it('fails closed on status failure and renders backend text without executing it', async () => {
    await start(vi.fn().mockRejectedValue(new Error('disconnected')));
    expect((document.getElementById('constitute') as HTMLButtonElement).disabled).toBe(true);
    expect(document.getElementById('status')?.textContent).toContain('could not verify');
    await start(vi.fn().mockResolvedValue({ phase: 'blocked', constitution_available: false, message: '<img src=x onerror=alert(1)>' }));
    expect(document.getElementById('status')?.querySelector('img')).toBeNull();
    expect(document.getElementById('status')?.textContent).toBe('<img src=x onerror=alert(1)>');
  });
});
