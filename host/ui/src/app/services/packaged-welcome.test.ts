import html from '../../../../crates/welcome/public/welcome.html?raw';
import script from '../../../../crates/welcome/public/welcome.js?raw';
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
  it('follows the new-user storyboard while preparation runs, without fabricated percentages', async () => {
    vi.useFakeTimers();
    const initial = { phase: 'initializing', stage: 'initializing-host', constitution_available: false };
    const preparing = { phase: 'provisioning', stage: 'loading-core-schema', preparing_new_space: true, constitution_available: false };
    const invoke = vi.fn().mockResolvedValue(initial);
    await start(invoke);
    expect(document.body.dataset.view).toBe('discovery');
    expect(document.getElementById('activity-message')?.textContent).toContain('Looking for your personal space');
    invoke.mockResolvedValue(preparing);
    await vi.advanceTimersByTimeAsync(1000);
    expect(document.body.dataset.view).toBe('invitation');
    const usualWay = document.getElementById('usual-way') as HTMLElement;
    expect(usualWay.hidden).toBe(false);
    expect(usualWay.querySelector('input, button')).toBeNull();
    expect(document.getElementById('preparation-progress')?.hasAttribute('aria-valuenow')).toBe(false);
    await vi.advanceTimersByTimeAsync(5000);
    expect(document.body.dataset.view).toBe('invitation');
    await vi.advanceTimersByTimeAsync(10000);
    expect(document.body.dataset.view).toBe('privacy');
    expect(usualWay.hidden).toBe(true);
    invoke.mockResolvedValue({ ...preparing, phase: 'checking-space', stage: 'verifying-space-definitions' });
    await vi.advanceTimersByTimeAsync(1000);
    expect(document.body.dataset.view).toBe('finalizing');
    expect(document.getElementById('activity-message')?.textContent).toBe('Finalizing your space…');
    invoke.mockResolvedValue({ phase: 'foundation-present', preparing_new_space: true, constitution_available: true });
    await vi.advanceTimersByTimeAsync(1000);
    expect(document.body.dataset.view).toBe('naming');
    expect((document.getElementById('personal-name') as HTMLFieldSetElement).hidden).toBe(false);
    expect(document.getElementById('constitute')?.textContent).toContain('Make this space mine');
    expect((document.getElementById('open-navigator') as HTMLButtonElement).hidden).toBe(true);
    expect(vi.getTimerCount()).toBe(0);
  });

  it('gives the comparison time to be read even when setup reaches finalization quickly', async () => {
    vi.useFakeTimers();
    const invoke = vi.fn().mockResolvedValue({ phase: 'provisioning', stage: 'loading-core-schema', preparing_new_space: true });
    await start(invoke);
    invoke.mockResolvedValue({ phase: 'checking-space', stage: 'verifying-space-definitions', preparing_new_space: true });
    await vi.advanceTimersByTimeAsync(14000);
    expect(document.body.dataset.view).toBe('invitation');
    await vi.advanceTimersByTimeAsync(1000);
    expect(document.body.dataset.view).toBe('finalizing');
    invoke.mockResolvedValue({ phase: 'foundation-present', constitution_available: true });
    await vi.advanceTimersByTimeAsync(1000);
    expect(document.body.dataset.view).toBe('naming');
  });

  it('keeps a returning user in discovery until the existing Space is verified', async () => {
    vi.useFakeTimers();
    const invoke = vi.fn().mockResolvedValueOnce({ phase: 'checking-space', stage: 'verifying-space-definitions', preparing_new_space: false })
      .mockResolvedValueOnce({ phase: 'association-present', navigator_available: true, constitution_available: false });
    await start(invoke);
    expect(document.body.dataset.view).toBe('discovery');
    expect((document.getElementById('usual-way') as HTMLElement).hidden).toBe(true);
    await vi.advanceTimersByTimeAsync(1000);
    expect(document.body.dataset.view).toBe('returning');
    expect(invoke.mock.calls).toEqual([['welcome_status'], ['welcome_status']]);
  });

  it('shows naming immediately when a fast preparation is already ready', async () => {
    vi.useFakeTimers();
    await start(vi.fn().mockResolvedValue({ phase: 'foundation-present', preparing_new_space: true, constitution_available: true }));
    expect(document.body.dataset.view).toBe('naming');
    expect(vi.getTimerCount()).toBe(0);
  });

  it.each([
    { phase: 'foundation-present', constitution_available: true, visible: true },
    { phase: 'association-present', constitution_available: false, visible: false },
    { phase: 'blocked', constitution_available: false, visible: false },
  ])('waits for verified startup before offering creation: $phase', async ready => {
    vi.useFakeTimers();
    let resolveStatus!: (status: object) => void;
    const invoke = vi.fn()
      .mockImplementationOnce(() => new Promise(resolve => { resolveStatus = resolve; }))
      .mockResolvedValueOnce({ phase: 'provisioning', constitution_available: false })
      .mockResolvedValueOnce({ phase: 'checking-space', constitution_available: true })
      .mockResolvedValueOnce(ready);
    await start(invoke);
    const create = document.getElementById('constitute') as HTMLButtonElement;
    expect(create.hidden).toBe(true);
    resolveStatus({ phase: 'initializing', stage: 'initializing-host', constitution_available: false });
    await vi.advanceTimersByTimeAsync(0);
    expect(create.hidden).toBe(true);
    for (let step = 0; step < 2; step++) {
      await vi.advanceTimersByTimeAsync(1000);
      expect(create.hidden).toBe(true);
      expect(create.disabled).toBe(true);
      expect((document.getElementById('personal-name') as HTMLFieldSetElement).hidden).toBe(true);
    }
    await vi.advanceTimersByTimeAsync(1000);
    expect(create.hidden).toBe(!ready.visible);
    expect(create.disabled).toBe(!ready.visible);
    expect(invoke.mock.calls.every(call => call[0] === 'welcome_status')).toBe(true);
  });

  it('explains incompatibility and creates a separate environment only after an explicit click', async () => {
    let complete!: () => void;
    const invoke = vi.fn().mockResolvedValueOnce({ phase: 'blocked', stage: 'failed', constitution_available: false,
      new_environment_available: true, incompatibility: 'Persisted DNA differs from packaged DNA', message: 'Blocked.' })
      .mockImplementationOnce(() => new Promise<void>(resolve => { complete = resolve; }));
    await start(invoke);
    const create = document.getElementById('new-environment') as HTMLButtonElement;
    expect(create.hidden).toBe(false);
    expect((document.getElementById('constitute') as HTMLButtonElement).hidden).toBe(true);
    expect(document.getElementById('status')?.textContent).toContain('Persisted DNA differs');
    expect(document.getElementById('status')?.textContent).toContain('preserved');
    expect(invoke.mock.calls).toEqual([['welcome_status']]);
    create.click();
    create.dispatchEvent(new MouseEvent('click'));
    expect(create.disabled).toBe(true);
    expect(invoke.mock.calls).toEqual([['welcome_status'], ['create_new_environment']]);
    complete();
  });

  it('does not offer a new environment for an ordinary startup failure', async () => {
    const invoke = vi.fn().mockResolvedValueOnce({ phase: 'blocked', stage: 'failed', message: 'Storage could not open.' });
    await start(invoke);
    expect((document.getElementById('new-environment') as HTMLButtonElement).hidden).toBe(true);
    document.getElementById('new-environment')?.dispatchEvent(new MouseEvent('click'));
    expect(invoke.mock.calls).toEqual([['welcome_status']]);
  });

  it('waits for an explicit choice before launching the existing Space', async () => {
    let finish!: () => void;
    const invoke = vi.fn().mockResolvedValueOnce({ phase: 'association-present', navigator_available: true,
      new_environment_available: true, constitution_available: false, message: 'Choose how to continue.' })
      .mockImplementationOnce(() => new Promise<void>(resolve => { finish = resolve; }));
    await start(invoke);
    const launch = document.getElementById('open-navigator') as HTMLButtonElement;
    const reset = document.getElementById('new-environment') as HTMLButtonElement;
    expect(launch.hidden).toBe(false);
    expect(launch.textContent).toContain('Enter my space');
    expect(reset.hidden).toBe(false);
    expect(reset.textContent).toBe('Reset and restart my Space');
    expect(document.getElementById('reset-description')?.textContent).toContain('kept');
    expect(invoke.mock.calls).toEqual([['welcome_status']]);
    launch.click();
    launch.dispatchEvent(new MouseEvent('click'));
    reset.dispatchEvent(new MouseEvent('click'));
    expect(launch.disabled).toBe(true);
    expect(reset.disabled).toBe(true);
    expect(invoke.mock.calls).toEqual([['welcome_status'], ['open_space_navigator']]);
    finish();
  });

  it('retries a failed handoff without repeating constitution', async () => {
    const invoke = vi.fn().mockResolvedValueOnce({ phase: 'association-present', navigator_available: true,
      new_environment_available: true, constitution_available: false, message: 'Open.' })
      .mockRejectedValueOnce('Navigator selection failed.').mockResolvedValueOnce(undefined);
    await start(invoke);
    const retry = document.getElementById('open-navigator') as HTMLButtonElement;
    retry.click();
    await vi.waitFor(() => expect(document.getElementById('status')?.textContent).toBe('Navigator selection failed.'));
    expect(retry.hidden).toBe(false);
    expect(retry.disabled).toBe(false);
    expect((document.getElementById('new-environment') as HTMLButtonElement).disabled).toBe(false);
    retry.click();
    await vi.waitFor(() => expect(invoke.mock.calls).toEqual([['welcome_status'], ['open_space_navigator'], ['open_space_navigator']]));
    expect((document.getElementById('constitute') as HTMLButtonElement).hidden).toBe(true);
  });

  it('resets and restarts a ready Space only on request and restores both choices on failure', async () => {
    let reject!: (error: string) => void;
    const invoke = vi.fn().mockResolvedValueOnce({ phase: 'association-present', navigator_available: true,
      new_environment_available: true, constitution_available: false, message: 'Open.' })
      .mockImplementationOnce(() => new Promise<void>((_, fail) => { reject = fail; }));
    await start(invoke);
    const reset = document.getElementById('new-environment') as HTMLButtonElement;
    const launch = document.getElementById('open-navigator') as HTMLButtonElement;
    reset.click();
    reset.dispatchEvent(new MouseEvent('click'));
    launch.dispatchEvent(new MouseEvent('click'));
    expect(invoke.mock.calls).toEqual([['welcome_status'], ['create_new_environment']]);
    expect(reset.disabled).toBe(true);
    expect(launch.disabled).toBe(true);
    expect(document.getElementById('activity-stage')?.textContent).toContain('Restarting');
    reject('Restart could not be requested.');
    await vi.waitFor(() => expect(document.getElementById('status')?.textContent).toBe('Restart could not be requested.'));
    expect(reset.disabled).toBe(false);
    expect(launch.disabled).toBe(false);
    expect((document.getElementById('activity') as HTMLElement).hidden).toBe(true);
  });

  it('waits for a launch choice after Complete and fresh saved association confirmation', async () => {
    const invoke = vi.fn().mockResolvedValueOnce({ phase: 'foundation-present', constitution_available: true, message: 'Ready.' })
      .mockResolvedValueOnce('complete')
      .mockResolvedValueOnce({ phase: 'association-present', navigator_available: true, new_environment_available: true,
        constitution_available: false, message: 'Open.' })
      .mockResolvedValueOnce(undefined);
    await start(invoke);
    (document.getElementById('last-name') as HTMLInputElement).value='Example';
    document.getElementById('constitute')?.click();
    await vi.waitFor(() => expect((document.getElementById('open-navigator') as HTMLButtonElement).hidden).toBe(false));
    expect(invoke.mock.calls.map(call => call[0])).toEqual(['welcome_status', 'constitute_personal_space', 'welcome_status']);
    document.getElementById('open-navigator')?.click();
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
    await vi.waitFor(() => expect(document.querySelector('h1')?.textContent).toBe('You’re on\nthe MAP.'));
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
    expect(document.body.dataset.view).toBe('confirming');
    expect((document.getElementById('open-navigator') as HTMLButtonElement).hidden).toBe(true);
    button.dispatchEvent(new MouseEvent('click'));
    expect(invoke.mock.calls.filter(call => call[0] === 'constitute_personal_space')).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1000);
    expect(document.querySelector('h1')?.textContent).toBe('You’re on\nthe MAP.');
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
    const details = document.getElementById('startup-details') as HTMLDetailsElement;
    const elapsed = document.getElementById('activity-elapsed')!;
    expect(details.open).toBe(false);
    expect(elapsed.closest('details, [hidden]')).toBeNull();
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
    expect(document.querySelector('h1')?.textContent).toBe('Welcome\nback to MAP.');
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


describe('saved environment management', () => {
  const ready = { phase: 'association-present', navigator_available: true, new_environment_available: true,
    constitution_available: false, environment_management_available: true, message: 'Choose how to continue.' };
  const active = { id: 'environment-current', active: true, created_seconds: 1795210000, size_bytes: 2048 };
  const old = { id: 'local-constitution', active: false, created_seconds: 1795200000, size_bytes: 4096 };
  async function open(invoke: ReturnType<typeof vi.fn>) {
    await start(invoke);
    const panel = document.getElementById('saved-environments') as HTMLDetailsElement;
    expect(panel.hidden).toBe(false);
    panel.open = true; panel.dispatchEvent(new Event('toggle'));
    await vi.waitFor(() => expect(document.querySelectorAll('.saved-environment')).toHaveLength(2));
  }
  it('inventories environments without switching and protects the active entry', async () => {
    const invoke = vi.fn().mockResolvedValueOnce(ready).mockResolvedValueOnce([active, old]);
    await open(invoke);
    expect(invoke.mock.calls).toEqual([['welcome_status'], ['saved_environments']]);
    const current = document.querySelector('[data-environment-id="environment-current"]')!;
    expect(current.textContent).toContain('Active');
    expect(current.textContent).toContain('2 KB');
    expect(current.querySelector('button')).toBeNull();
    expect(document.getElementById('environment-list')?.textContent).toContain('Original environment');
  });
  it('reactivates only on request, sends only the environment ID and suppresses overlapping actions', async () => {
    const invoke = vi.fn().mockResolvedValueOnce(ready).mockResolvedValueOnce([active, old]).mockResolvedValueOnce(undefined);
    await open(invoke);
    const activate = [...document.querySelectorAll<HTMLButtonElement>('#environment-list button')].find(b => b.textContent === 'Reactivate and restart')!;
    activate.click(); activate.dispatchEvent(new MouseEvent('click'));
    document.getElementById('open-navigator')?.dispatchEvent(new MouseEvent('click'));
    await Promise.resolve();
    expect(invoke.mock.calls).toEqual([['welcome_status'], ['saved_environments'], ['activate_saved_environment', { id: old.id }]]);
    expect((document.getElementById('open-navigator') as HTMLButtonElement).disabled).toBe(true);
  });
  it('confirms recoverable cleanup and refreshes the list without touching the active environment', async () => {
    const invoke = vi.fn().mockResolvedValueOnce(ready).mockResolvedValueOnce([active, old])
      .mockResolvedValueOnce(undefined).mockResolvedValueOnce([active]);
    await open(invoke);
    const trash = [...document.querySelectorAll<HTMLButtonElement>('#environment-list button')].find(b => b.textContent === 'Move to Trash')!;
    trash.click();
    expect(invoke).toHaveBeenCalledTimes(2);
    expect(document.querySelector('.environment-confirmation')?.textContent).toContain('recovered');
    const confirm = [...document.querySelectorAll<HTMLButtonElement>('#environment-list button')].find(b => b.textContent === 'Confirm move to Trash')!;
    confirm.click(); confirm.dispatchEvent(new MouseEvent('click'));
    await vi.waitFor(() => expect(document.querySelectorAll('.saved-environment')).toHaveLength(1));
    expect(invoke.mock.calls).toEqual([['welcome_status'], ['saved_environments'], ['trash_saved_environment', { id: old.id }], ['saved_environments']]);
  });
});
