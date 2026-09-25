import { afterEach, expect, it, vi } from 'vitest';
import { NavigationProfile } from './navigation-profile';

afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks(); localStorage.clear(); });

it('is opt-in and separates queue wait from attributed commands while bounding history', () => {
  expect(NavigationProfile.start()).toBeUndefined();
  localStorage.setItem('map.profileNavigation', '1');
  let now = 0;
  let run: string;
  vi.stubGlobal('performance', {
    now: () => now,
    mark: (_: string, options: { detail: string }) => { run = options.detail; },
    clearMarks: vi.fn(), clearMeasures: vi.fn(),
    getEntriesByName: (name: string) => name === 'map.navigation.ipc' ? [
      { startTime: 12, duration: 8, detail: { run, requestId: 42, command: 'Holon.Read.GetRelatedHolons' } },
      { startTime: 12, duration: 1000, detail: { run: 'unrelated', requestId: 41 } },
    ] : [],
  });
  vi.spyOn(console, 'info').mockImplementation(() => {});
  vi.spyOn(console, 'table').mockImplementation(() => {});
  for (let index = 0; index < 6; index++) {
    now = 0;
    const profile = NavigationProfile.start()!;
    now = 10; profile.begin();
    now = 12; profile.next('target retrieval');
    now = 20; profile.finish('new node');
    profile.finish('ignored');
  }
  const reports = JSON.parse(localStorage.getItem('map.navigationProfiles')!);
  expect(reports).toHaveLength(5);
  expect(reports[4]).toMatchObject({ outcome: 'new node', totalMs: 20,
    phases: [{ phase: 'queue wait', ms: 10 }, { phase: 'relationship name', ms: 2 }, { phase: 'target retrieval', ms: 8 }],
    commands: [{ requestId: 42, command: 'Holon.Read.GetRelatedHolons', phase: 'target retrieval', ms: 8 }],
  });
});
