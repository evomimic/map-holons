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
  it('sends only naming data and suppresses duplicate submissions', async () => {
    let rejectRequest!: (reason: string) => void;
    const invoke = vi.fn().mockResolvedValueOnce({ phase: 'foundation-present', constitution_available: true, message: 'Eligible fixture.' })
      .mockImplementationOnce(() => new Promise((_, reject) => { rejectRequest = reject; }));
    await start(invoke);
    const names = document.getElementById('personal-name') as HTMLFieldSetElement;
    expect(names.hidden).toBe(false);
    expect(names.disabled).toBe(false);
    (document.getElementById('last-name') as HTMLInputElement).value = 'van den Berg';
    (document.getElementById('first-name') as HTMLInputElement).value = 'Élodie';
    const button = document.getElementById('constitute') as HTMLButtonElement;
    button.click();
    button.dispatchEvent(new MouseEvent('click'));
    expect(invoke.mock.calls).toEqual([
      ['welcome_status'],
      ['constitute_personal_space', { input: { lastName: 'van den Berg', firstName: 'Élodie' } }],
    ]);
    expect(names.disabled).toBe(true);
    rejectRequest('Shared acquisition enforcement is unavailable.');
    await Promise.resolve();
    await Promise.resolve();
    expect(document.getElementById('status')?.textContent).toBe('Shared acquisition enforcement is unavailable.');
    expect(button.disabled).toBe(true);
    expect((document.getElementById('last-name') as HTMLInputElement).value).toBe('van den Berg');
  });

  it('requires a last name before sending a request', async () => {
    const invoke = vi.fn().mockResolvedValue({ phase: 'foundation-present', constitution_available: true, message: 'Eligible fixture.' });
    await start(invoke);
    (document.getElementById('constitute') as HTMLButtonElement).click();
    expect(invoke.mock.calls).toEqual([['welcome_status']]);
  });

  it('does not submit when a disabled control is forcibly dispatched', async () => {
    const invoke = vi.fn().mockResolvedValue({ phase: 'foundation-present', constitution_available: false, message: 'Unavailable.' });
    await start(invoke);
    (document.getElementById('last-name') as HTMLInputElement).value = 'Example';
    document.getElementById('constitute')?.dispatchEvent(new MouseEvent('click'));
    expect(invoke.mock.calls).toEqual([['welcome_status']]);
    expect((document.getElementById('personal-name') as HTMLFieldSetElement).hidden).toBe(true);
  });
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
