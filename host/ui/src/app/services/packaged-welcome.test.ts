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
  it('auto-advances only during confirmed new-Space preparation, every 15 seconds, and stops at the last card', async () => {
    vi.useFakeTimers();
    const invoke = vi.fn().mockResolvedValue({ phase: 'initializing', preparing_new_space: false });
    await start(invoke);
    const dots = [...document.querySelectorAll<HTMLButtonElement>('#story-dots button')];
    await vi.advanceTimersByTimeAsync(30000);
    expect(dots[0].getAttribute('aria-current')).toBe('true');
    invoke.mockResolvedValue({ phase: 'provisioning', stage: 'loading-core-schema', preparing_new_space: true });
    await vi.advanceTimersByTimeAsync(1000);
    await vi.advanceTimersByTimeAsync(14999);
    expect(dots[0].getAttribute('aria-current')).toBe('true');
    await vi.advanceTimersByTimeAsync(1);
    expect(dots[1].getAttribute('aria-current')).toBe('true');
    await vi.advanceTimersByTimeAsync(15000);
    expect(dots[2].getAttribute('aria-current')).toBe('true');
    await vi.advanceTimersByTimeAsync(15000);
    expect(dots[3].getAttribute('aria-current')).toBe('true');
    await vi.advanceTimersByTimeAsync(60000);
    expect(dots[3].getAttribute('aria-current')).toBe('true');
  });

  it.each(['dot', 'keyboard', 'wheel', 'touch', 'focus', 'pointer'])('permanently stops auto-advance after the first manual %s interaction', async interaction => {
    vi.useFakeTimers();
    const invoke = vi.fn().mockResolvedValue({ phase: 'provisioning', preparing_new_space: true });
    await start(invoke);
    await vi.advanceTimersByTimeAsync(14000);
    const pages = document.getElementById('story-pages')!;
    const dots = [...document.querySelectorAll<HTMLButtonElement>('#story-dots button')];
    let selected = 0;
    if (interaction === 'dot') { dots[2].click(); selected = 2; }
    if (interaction === 'keyboard') { pages.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true })); selected = 1; }
    if (interaction === 'wheel') { document.body.dispatchEvent(new WheelEvent('wheel', { deltaX: 100, bubbles: true, cancelable: true })); selected = 1; }
    if (interaction === 'touch') {
      const event = new Event('touchstart', { bubbles: true });
      Object.defineProperty(event, 'touches', { value: [{ clientX: 100, clientY: 50 }] });
      pages.dispatchEvent(event);
    }
    if (interaction === 'focus') dots[0].focus();
    if (interaction === 'pointer') pages.dispatchEvent(new Event('pointerdown', { bubbles: true }));
    await vi.advanceTimersByTimeAsync(60000);
    expect(dots[selected].getAttribute('aria-current')).toBe('true');
    // Repeated lifecycle renders and even temporary disappearance cannot rearm it.
    invoke.mockResolvedValue({ phase: 'checking-space', preparing_new_space: false });
    await vi.advanceTimersByTimeAsync(1000);
    invoke.mockResolvedValue({ phase: 'checking-space', preparing_new_space: true });
    await vi.advanceTimersByTimeAsync(31000);
    expect(dots[selected].getAttribute('aria-current')).toBe('true');
  });

  it.each([
    { phase: 'foundation-present', constitution_available: true },
    { phase: 'association-present', navigator_available: true },
    { phase: 'blocked', incompatibility: 'Incompatible environment' },
  ])('cancels pending auto-advance when startup leaves preparation: $phase', async outcome => {
    vi.useFakeTimers();
    const invoke = vi.fn().mockResolvedValue({ phase: 'provisioning', preparing_new_space: true });
    await start(invoke);
    await vi.advanceTimersByTimeAsync(14000);
    invoke.mockResolvedValue(outcome);
    await vi.advanceTimersByTimeAsync(1000);
    const selected = document.querySelector('#story-dots [aria-current]');
    expect((document.getElementById('welcome-story') as HTMLElement).hidden).toBe(true);
    await vi.advanceTimersByTimeAsync(60000);
    expect(vi.getTimerCount()).toBe(0);
    expect(document.querySelector('#story-dots [aria-current]')).toBe(selected);
  });

  it('keeps the reader’s selected message while startup advances and naming becomes available', async () => {
    vi.useFakeTimers();
    const preparing = { phase: 'provisioning', stage: 'loading-core-schema', preparing_new_space: true, constitution_available: false };
    const invoke = vi.fn().mockResolvedValue(preparing);
    await start(invoke);
    const dots = [...document.querySelectorAll<HTMLButtonElement>('#story-dots button')];
    dots[2].click();
    expect(dots[2].getAttribute('aria-current')).toBe('true');
    expect(document.getElementById('story-privacy')?.getAttribute('aria-hidden')).toBe('false');
    const usualWay = document.getElementById('usual-way') as HTMLElement;
    expect(usualWay.querySelector('input, button')).toBeNull();
    expect(document.getElementById('preparation-progress')?.hasAttribute('aria-valuenow')).toBe(false);
    invoke.mockResolvedValue({ ...preparing, phase: 'checking-space', stage: 'verifying-space-definitions' });
    await vi.advanceTimersByTimeAsync(30000);
    expect(dots[2].getAttribute('aria-current')).toBe('true');
    expect(document.getElementById('activity-message')?.textContent).toBe('Verifying packaged definitions');
    expect(document.getElementById('activity-elapsed')?.textContent).toContain('Total: 0:30');
    invoke.mockResolvedValue({ phase: 'foundation-present', preparing_new_space: true, constitution_available: true });
    await vi.advanceTimersByTimeAsync(1000);
    expect(document.body.dataset.view).toBe('naming');
    expect((document.getElementById('personal-name') as HTMLFieldSetElement).hidden).toBe(false);
    expect(document.getElementById('constitute')?.textContent).toContain('Make this space mine');
    expect(dots[2].getAttribute('aria-current')).toBe('true');
    expect((document.getElementById('welcome-story') as HTMLElement).hidden).toBe(true);
    expect(vi.getTimerCount()).toBe(0);
    expect(invoke.mock.calls.every(call => call[0] === 'welcome_status')).toBe(true);
  });

  it('limits a momentum wheel gesture to one page and allows the next gesture after it settles', async () => {
    vi.useFakeTimers();
    await start(vi.fn().mockResolvedValue({ phase: 'provisioning', preparing_new_space: true }));
    const pages = document.getElementById('story-pages')!;
    const dots = [...document.querySelectorAll<HTMLButtonElement>('#story-dots button')];
    const wheel = (deltaX: number, deltaY = 0) => pages.dispatchEvent(new WheelEvent('wheel', { deltaX, deltaY, bubbles: true, cancelable: true }));
    for (let index = 0; index < 10; index++) { wheel(150); await vi.advanceTimersByTimeAsync(30); }
    expect(dots[1].getAttribute('aria-current')).toBe('true');
    expect((document.getElementById('story-invitation') as HTMLElement).hidden).toBe(false);
    expect((document.getElementById('story-own') as HTMLElement).hidden).toBe(true);
    await vi.advanceTimersByTimeAsync(250);
    wheel(600); expect(dots[2].getAttribute('aria-current')).toBe('true');
    await vi.advanceTimersByTimeAsync(250);
    wheel(-600); expect(dots[1].getAttribute('aria-current')).toBe('true');
    expect(wheel(0, 200)).toBe(true); // Ordinary vertical scrolling remains available.
  });

  it('pages from the message, dots, or background without moving keyboard focus or clicking', async () => {
    vi.useFakeTimers();
    const invoke = vi.fn().mockResolvedValue({ phase: 'provisioning', preparing_new_space: true });
    await start(invoke);
    const dots = [...document.querySelectorAll<HTMLButtonElement>('#story-dots button')];
    const focus = document.activeElement;
    const wheel = (target: Element) => target.dispatchEvent(new WheelEvent('wheel', { deltaX: 100, bubbles: true, cancelable: true }));
    wheel(document.body);
    expect(dots[1].getAttribute('aria-current')).toBe('true');
    await vi.advanceTimersByTimeAsync(250);
    wheel(document.getElementById('story-dots')!);
    expect(dots[2].getAttribute('aria-current')).toBe('true');
    await vi.advanceTimersByTimeAsync(250);
    wheel(document.getElementById('story-privacy')!.querySelector('h2')!);
    expect(dots[3].getAttribute('aria-current')).toBe('true');
    expect(document.activeElement).toBe(focus);
    invoke.mockResolvedValue({ phase: 'association-present', navigator_available: true });
    await vi.advanceTimersByTimeAsync(1000);
    document.body.dispatchEvent(new WheelEvent('wheel', { deltaX: -100, bubbles: true, cancelable: true }));
    expect(dots[3].getAttribute('aria-current')).toBe('true');
  });

  it('accepts a fresh swipe after decaying momentum without a click or a fully idle wheel stream', async () => {
    vi.useFakeTimers();
    await start(vi.fn().mockResolvedValue({ phase: 'provisioning', preparing_new_space: true }));
    const pages = document.getElementById('story-pages')!;
    const dots = [...document.querySelectorAll<HTMLButtonElement>('#story-dots button')];
    const wheel = (deltaX: number) => pages.dispatchEvent(new WheelEvent('wheel', { deltaX, bubbles: true, cancelable: true }));
    for (const delta of [100, 70, 35, 10, 2]) { wheel(delta); await vi.advanceTimersByTimeAsync(25); }
    expect(dots[1].getAttribute('aria-current')).toBe('true');
    wheel(100);
    expect(dots[2].getAttribute('aria-current')).toBe('true');
    for (const delta of [75, 40, 10, 2]) { await vi.advanceTimersByTimeAsync(25); wheel(delta); }
    expect(dots[2].getAttribute('aria-current')).toBe('true');
  });

  it('supports dots, keyboard and one-page touch swipes without invoking startup actions', async () => {
    vi.useFakeTimers();
    const invoke = vi.fn().mockResolvedValue({ phase: 'provisioning', preparing_new_space: true });
    await start(invoke);
    const pages = document.getElementById('story-pages')!;
    const dots = [...document.querySelectorAll<HTMLButtonElement>('#story-dots button')];
    const key = (value: string) => pages.dispatchEvent(new KeyboardEvent('keydown', { key: value, bubbles: true, cancelable: true }));
    dots[1].focus(); dots[1].click();
    dots[1].dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
    expect(document.activeElement).toBe(dots[2]);
    key('End'); expect(dots[3].getAttribute('aria-current')).toBe('true');
    key('ArrowRight'); expect(dots[3].getAttribute('aria-current')).toBe('true');
    key('Home'); expect(dots[0].getAttribute('aria-current')).toBe('true');
    key('ArrowLeft'); expect(dots[0].getAttribute('aria-current')).toBe('true');
    const touch = (type: string, x: number, y: number) => {
      const event = new Event(type, { bubbles: true });
      Object.defineProperty(event, type === 'touchstart' ? 'touches' : 'changedTouches', { value: [{ clientX: x, clientY: y }] });
      pages.dispatchEvent(event);
    };
    touch('touchstart', 600, 50); touch('touchend', 0, 50);
    expect(dots[1].getAttribute('aria-current')).toBe('true');
    touch('touchstart', 0, 50); touch('touchend', 600, 50);
    expect(dots[0].getAttribute('aria-current')).toBe('true');
    touch('touchstart', 50, 600); touch('touchend', 50, 0);
    expect(dots[0].getAttribute('aria-current')).toBe('true');
    expect(invoke.mock.calls).toEqual([['welcome_status']]);
  });

  it.each([
    { phase: 'foundation-present', constitution_available: true },
    { phase: 'association-present', navigator_available: true },
  ])('removes the informational pages and dots from setup and returning-user layouts: $phase', async lifecycle => {
    vi.useFakeTimers();
    await start(vi.fn().mockResolvedValue(lifecycle));
    expect((document.getElementById('welcome-story') as HTMLElement).hidden).toBe(true);
  });

  it('keeps failures visible independently of the selected informational card', async () => {
    vi.useFakeTimers();
    const invoke = vi.fn().mockResolvedValue({ phase: 'provisioning', preparing_new_space: true });
    await start(invoke);
    const dot = document.querySelectorAll<HTMLButtonElement>('#story-dots button')[3];
    dot.click();
    invoke.mockResolvedValue({ phase: 'blocked', incompatibility: 'Older environment', new_environment_available: true });
    await vi.advanceTimersByTimeAsync(1000);
    expect(dot.getAttribute('aria-current')).toBe('true');
    expect((document.getElementById('attention') as HTMLElement).hidden).toBe(false);
    expect(document.getElementById('recovery-message')?.textContent).toContain('cannot open this environment');
    expect((document.getElementById('new-environment') as HTMLButtonElement).hidden).toBe(false);
    document.querySelectorAll<HTMLButtonElement>('#story-dots button')[0].click();
    expect(document.body.dataset.view).toBe('recovery');
    expect((document.getElementById('attention') as HTMLElement).hidden).toBe(false);
  });

  it('keeps discovery static and never offers the new-Space carousel to a returning user', async () => {
    vi.useFakeTimers();
    const invoke = vi.fn().mockResolvedValueOnce({ phase: 'checking-space', stage: 'verifying-space-definitions', preparing_new_space: false })
      .mockResolvedValueOnce({ phase: 'association-present', navigator_available: true, constitution_available: false });
    await start(invoke);
    expect(document.body.dataset.view).toBe('discovery');
    expect((document.getElementById('discovery-story') as HTMLElement).hidden).toBe(false);
    expect(document.getElementById('discovery-story')?.textContent).toContain('A space');
    expect((document.getElementById('welcome-story') as HTMLElement).hidden).toBe(true);
    await vi.advanceTimersByTimeAsync(1000);
    expect(document.body.dataset.view).toBe('returning');
    expect((document.getElementById('discovery-story') as HTMLElement).hidden).toBe(true);
    expect((document.getElementById('welcome-story') as HTMLElement).hidden).toBe(true);
    expect(invoke.mock.calls).toEqual([['welcome_status'], ['welcome_status']]);
  });

  it('offers the carousel only after explicit confirmation that a new Space is being prepared', async () => {
    vi.useFakeTimers();
    let resolveStatus!: (status: object) => void;
    const invoke = vi.fn().mockImplementationOnce(() => new Promise(resolve => { resolveStatus = resolve; }));
    await start(invoke);
    expect((document.getElementById('welcome-story') as HTMLElement).hidden).toBe(true);
    expect((document.getElementById('discovery-story') as HTMLElement).hidden).toBe(false);
    invoke.mockResolvedValue({ phase: 'provisioning', preparing_new_space: true });
    resolveStatus({ phase: 'provisioning', preparing_new_space: false });
    await vi.advanceTimersByTimeAsync(0);
    expect((document.getElementById('welcome-story') as HTMLElement).hidden).toBe(true);
    await vi.advanceTimersByTimeAsync(1000);
    expect((document.getElementById('welcome-story') as HTMLElement).hidden).toBe(false);
    expect((document.getElementById('discovery-story') as HTMLElement).hidden).toBe(true);
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
    expect(document.querySelector('.saved-environment[data-confirming]')?.textContent).toContain('Recoverable');
    const confirm = [...document.querySelectorAll<HTMLButtonElement>('#environment-list button')].find(b => b.textContent === 'Confirm move to Trash')!;
    confirm.click(); confirm.dispatchEvent(new MouseEvent('click'));
    await vi.waitFor(() => expect(document.querySelectorAll('.saved-environment')).toHaveLength(1));
    expect(invoke.mock.calls).toEqual([['welcome_status'], ['saved_environments'], ['trash_saved_environment', { id: old.id }], ['saved_environments']]);
  });  it('replaces the selected environment action row for confirmation and restores it on cancel', async () => {
    vi.useFakeTimers();
    const invoke = vi.fn().mockImplementation(async command => command === 'welcome_status'
      ? { phase: 'association-present', navigator_available: true, environment_management_available: true }
      : [{ id: 'first', active: false, size_bytes: 100 }, { id: 'second', active: false, size_bytes: 200 }]);
    await start(invoke);
    document.getElementById('refresh-environments')!.click();
    await vi.advanceTimersByTimeAsync(0);
    const cards = [...document.querySelectorAll<HTMLElement>('.saved-environment')];
    const first = cards[0], second = cards[1];
    const originalActions = first.querySelector('.environment-actions')!;
    const trash = [...originalActions.querySelectorAll('button')].find(button => button.textContent === 'Move to Trash')!;
    const secondContent = second.innerHTML;
    const originalInfo = first.querySelector('p')!.textContent;
    const childCount = first.children.length;
    trash.click();
    const confirmation = first.querySelector('.environment-confirmation')!;
    expect(confirmation.parentElement).toBe(first);
    expect(originalActions.isConnected).toBe(false);
    expect(first.children.length).toBe(childCount);
    expect(second.innerHTML).toBe(secondContent);
    expect(second.querySelector('.environment-confirmation')).toBeNull();
    expect(document.activeElement?.textContent).toBe('Confirm move to Trash');
    [...confirmation.querySelectorAll('button')].find(button => button.textContent === 'Cancel')!.click();
    expect(first.querySelector('.environment-actions')).toBe(originalActions);
    expect(first.querySelector('p')!.textContent).toBe(originalInfo);
    expect(first.dataset.confirming).toBeUndefined();
    expect(document.activeElement).toBe(trash);
    expect(invoke.mock.calls).toEqual([['welcome_status'], ['saved_environments']]);
  });


});
