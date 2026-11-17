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
  it('automatically hands the existing acquired installation to Navigator once', async () => {
    const invoke = vi.fn().mockResolvedValueOnce({ phase: 'association-present', navigator_available: true, constitution_available: false, message: 'Open.' })
      .mockResolvedValueOnce(undefined);
    await start(invoke);
    await vi.waitFor(() => expect(invoke.mock.calls).toEqual([['welcome_status'], ['open_space_navigator']]));
    document.getElementById('constitute')?.dispatchEvent(new MouseEvent('click'));
    expect(invoke).toHaveBeenCalledTimes(2);
    expect(document.getElementById('activity-stage')?.textContent).toBe('Opening Space Navigator');
  });

  it('retries a failed handoff without repeating constitution', async () => {
    const invoke = vi.fn().mockResolvedValueOnce({ phase: 'association-present', navigator_available: true, constitution_available: false, message: 'Open.' })
      .mockRejectedValueOnce('Navigator selection failed.').mockResolvedValueOnce(undefined);
    await start(invoke);
    const retry = document.getElementById('open-navigator') as HTMLButtonElement;
    await vi.waitFor(() => expect(retry.hidden).toBe(false));
    expect(document.getElementById('status')?.textContent).toBe('Navigator selection failed.');
    retry.click();
    await vi.waitFor(() => expect(invoke.mock.calls).toEqual([['welcome_status'], ['open_space_navigator'], ['open_space_navigator']]));
    expect((document.getElementById('constitute') as HTMLButtonElement).hidden).toBe(true);
  });

  it('hands off after Complete and fresh saved association confirmation', async () => {
    const invoke = vi.fn().mockResolvedValueOnce({ phase: 'foundation-present', constitution_available: true, message: 'Ready.' })
      .mockResolvedValueOnce('complete')
      .mockResolvedValueOnce({ phase: 'association-present', navigator_available: true, constitution_available: false, message: 'Open.' })
      .mockResolvedValueOnce(undefined);
    await start(invoke);
    (document.getElementById('last-name') as HTMLInputElement).value='Example';
    document.getElementById('constitute')?.click();
    await vi.waitFor(() => expect(invoke.mock.calls.map(call => call[0])).toEqual(['welcome_status', 'constitute_personal_space', 'welcome_status', 'open_space_navigator']));
  });

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
    expect(document.getElementById('status')?.textContent).toContain('Creating');
    expect(document.getElementById('activity-stage')?.textContent).toBe('Creating your personal space');
    rejectRequest('Shared acquisition enforcement is unavailable.');
    await Promise.resolve();
    await Promise.resolve();
    expect(document.getElementById('status')?.textContent).toBe('Shared acquisition enforcement is unavailable.');
    expect(button.disabled).toBe(true);
    expect((document.getElementById('last-name') as HTMLInputElement).value).toBe('van den Berg');
  });

  it('creates through the backend and returns as the same acquired installation', async () => {
    const returning = { phase: 'association-present', constitution_available: false, message: 'Your existing personal space is open.' };
    const invoke = vi.fn()
      .mockResolvedValueOnce({ phase: 'foundation-present', constitution_available: true, message: 'Ready.' })
      .mockResolvedValueOnce('complete')
      .mockResolvedValueOnce(returning);
    await start(invoke);
    (document.getElementById('last-name') as HTMLInputElement).value = 'Example';
    (document.getElementById('constitute') as HTMLButtonElement).click();
    await vi.waitFor(() => expect(document.querySelector('h1')?.textContent).toBe('Welcome back to MAP'));
    expect((document.getElementById('constitute') as HTMLButtonElement).hidden).toBe(true);
    expect(invoke.mock.calls.map(call => call[0])).toEqual(['welcome_status', 'constitute_personal_space', 'welcome_status']);
    // A freshly loaded page on the next launch only reads status.
    const reopen = vi.fn().mockResolvedValue(returning);
    await start(reopen);
    document.getElementById('constitute')?.dispatchEvent(new MouseEvent('click'));
    expect(reopen.mock.calls).toEqual([['welcome_status']]);
  });

  it('keeps creation disabled after Complete while confirming saved state', async () => {
    vi.useFakeTimers();
    const ready = { phase: 'foundation-present', constitution_available: true, message: 'Ready.' };
    const invoke = vi.fn().mockResolvedValueOnce(ready)
      .mockResolvedValueOnce('complete').mockResolvedValueOnce(ready)
      .mockResolvedValueOnce({ phase: 'association-present', constitution_available: false, message: 'Your existing personal space is open.' });
    await start(invoke);
    (document.getElementById('last-name') as HTMLInputElement).value = 'Example';
    const button = document.getElementById('constitute') as HTMLButtonElement;
    button.click();
    await vi.advanceTimersByTimeAsync(0);
    expect(button.disabled).toBe(true);
    button.dispatchEvent(new MouseEvent('click'));
    expect(invoke.mock.calls.filter(call => call[0] === 'constitute_personal_space')).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1000);
    expect(document.querySelector('h1')?.textContent).toBe('Welcome back to MAP');
    expect(button.hidden).toBe(true);
  });

  it.each(['rejected', 'incomplete'])('retains %s work without submitting a new create', async outcome => {
    const invoke = vi.fn()
      .mockResolvedValueOnce({ phase: 'foundation-present', constitution_available: true, message: 'Ready.' })
      .mockResolvedValueOnce(outcome);
    await start(invoke);
    (document.getElementById('last-name') as HTMLInputElement).value = 'Example';
    (document.getElementById('constitute') as HTMLButtonElement).click();
    await vi.waitFor(() => expect(document.getElementById('status')?.textContent).toContain('resubmission'));
    document.getElementById('constitute')?.dispatchEvent(new MouseEvent('click'));
    expect(invoke).toHaveBeenCalledTimes(2);
    expect((document.getElementById('constitute') as HTMLButtonElement).disabled).toBe(true);
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
  it('shows live elapsed time and actual startup stages until readiness', async () => {
    vi.useFakeTimers();
    const invoke = vi.fn().mockResolvedValue({ phase: 'provisioning', stage: 'loading-core-schema', constitution_available: false, message: 'Preparing your space.' });
    await start(invoke);
    const activity = document.getElementById('activity') as HTMLDivElement;
    expect(activity.hidden).toBe(false);
    expect(document.getElementById('activity-stage')?.textContent).toBe('Loading the foundation');
    await vi.advanceTimersByTimeAsync(3000);
    expect(document.getElementById('activity-elapsed')?.textContent).toBe('This step: 0:03 · Total: 0:03');
    invoke.mockResolvedValueOnce({ phase: 'checking-space', stage: 'verifying-space-definitions', constitution_available: false, message: 'Checking your space.' })
      .mockResolvedValueOnce({ phase: 'association-present', constitution_available: false, message: 'Your personal space is open.' });
    await vi.advanceTimersByTimeAsync(1000);
    expect(document.getElementById('activity-stage')?.textContent).toBe('Verifying packaged definitions');
    expect(document.getElementById('activity-elapsed')?.textContent).toBe('This step: 0:00 · Total: 0:04');
    await vi.advanceTimersByTimeAsync(1000);
    expect(activity.hidden).toBe(true);
    expect(vi.getTimerCount()).toBe(0);
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
