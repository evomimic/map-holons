import '@angular/compiler';
import { Location } from '@angular/common';
import { SpyLocation, provideLocationMocks } from '@angular/common/testing';
import { TestBed } from '@angular/core/testing';
import { BrowserTestingModule, platformBrowserTesting } from '@angular/platform-browser/testing';
import { NavigationCancel, NavigationEnd, Router, provideRouter } from '@angular/router';
import { filter, firstValueFrom } from 'rxjs';
import { afterEach, beforeAll, expect, it } from 'vitest';
import { routes } from './app.routes';
import { CanvasHostComponent } from './components/canvas-host/canvas-host.component';
import { LoadHolonsDialogService } from './components/load-holons-dialog/load-holons-dialog.service';
import { ApplicationSessionService } from './services/application-session.service';
import { CanvasNavigationGuard } from './services/canvas-navigation-guard';

beforeAll(() => TestBed.initTestEnvironment(BrowserTestingModule, platformBrowserTesting()));
afterEach(() => TestBed.resetTestingModule());

it.each([
  ['/load-holons', 'imperative'], ['/load-holons-deprecated', 'imperative'],
  ['/load-holons', 'history'], ['/load-holons-deprecated', 'history'],
])('protects a normal Canvas load before navigating to %s through %s', async (destination, trigger) => {
  TestBed.configureTestingModule({ providers: [
    provideRouter(routes),
    provideLocationMocks(),
    { provide: LoadHolonsDialogService, useValue: {} },
    { provide: ApplicationSessionService, useValue: {} },
  ] });
  const router = TestBed.inject(Router);
  router.setUpLocationChangeListener();
  await router.navigateByUrl('/');
  const navigate = async () => {
    if (trigger === 'imperative') return router.navigateByUrl(destination);
    const completed = firstValueFrom(router.events.pipe(filter(event => event instanceof NavigationCancel || event instanceof NavigationEnd)));
    (TestBed.inject(Location) as SpyLocation).simulateUrlPop(destination);
    return await completed instanceof NavigationEnd;
  };
  const canvas = TestBed.runInInjectionContext(() => new CanvasHostComponent());
  let executing = true;
  Object.assign(canvas, { navigator: { canDismiss: () => !executing } });
  try {
    expect(await navigate()).toBe(false);
    expect(router.url).toBe('/');
    executing = false;
    expect(await navigate()).toBe(true);
    expect(router.url).toBe(destination);
    executing = true;
  } finally { canvas.ngOnDestroy(); }
  expect(TestBed.inject(CanvasNavigationGuard).canNavigate()).toBe(true);
});
