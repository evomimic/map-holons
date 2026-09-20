import { afterEach, expect, it, vi } from 'vitest';
import { StartupProfile } from './startup-profile';

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  localStorage.clear();
});

it('records phases, aggregates IPC costs, and retains bounded reports across reloads', () => {
  let now = 0;
  vi.stubGlobal('performance', {
    now: () => now,
    mark: vi.fn(),
    clearMarks: vi.fn(),
    clearMeasures: vi.fn(),
    getEntriesByName: () => [
      { detail: 'Holon.Read.GetPropertyValue', duration: 10 },
      { detail: 'Holon.Read.GetPropertyValue', duration: 30 },
    ],
  });
  vi.spyOn(console, 'info').mockImplementation(() => {});
  vi.spyOn(console, 'table').mockImplementation(() => {});
  for (let index = 0; index < 6; index++) {
    const profile = new StartupProfile();
    now += 5;
    profile.next('theme');
    now += 40;
    profile.finish('mounted');
    profile.finish('ignored');
  }
  const reports = JSON.parse(localStorage.getItem('map.startupProfiles')!);
  expect(reports).toHaveLength(5);
  expect(reports[4]).toMatchObject({
    outcome: 'mounted', totalMs: 45,
    phases: [{ phase: 'session-ready', ms: 5 }, { phase: 'theme', ms: 40 }],
    commands: [{ command: 'Holon.Read.GetPropertyValue', count: 2, totalMs: 40, maxMs: 30 }],
  });
});
