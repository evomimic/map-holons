import '@angular/compiler';
import { DestroyRef, Injector, runInInjectionContext } from '@angular/core';
import { NavigationEnd, Router } from '@angular/router';
import { Subject } from 'rxjs';
import { expect, it, vi } from 'vitest';
import { App } from './app';
import { ApplicationSessionService } from './services/application-session.service';
import { routes } from './app.routes';
import { LoadHolonsComponent } from './components/load-holons/load-holons.component';
import { DeprecatedLoadHolonsComponent } from './components/load-holons/deprecated-load-holons.component';

it('recognizes both loader deep links and subsequent navigation independently of launch experience', () => {
  const events = new Subject<NavigationEnd>();
  const injector = Injector.create({ providers: [
    { provide: Router, useValue: { url: '/load-holons?from=toolbar', events } },
    { provide: ApplicationSessionService, useValue: {} },
    { provide: DestroyRef, useValue: { onDestroy: () => () => {} } },
  ] });
  const app = runInInjectionContext(injector, () => new App()) as any;
  expect(app.loaderRoute()).toBe(true);
  expect(app.deprecatedLoaderRoute()).toBe(false);
  events.next(new NavigationEnd(1, '/load-holons-deprecated', '/load-holons-deprecated'));
  expect(app.loaderRoute()).toBe(false);
  expect(app.deprecatedLoaderRoute()).toBe(true);
  events.next(new NavigationEnd(2, '/', '/'));
  expect(app.loaderRoute()).toBe(false);
  expect(app.deprecatedLoaderRoute()).toBe(false);
  events.complete(); injector.destroy();
});

it('keeps the legacy route distinct and blocks departure while the new loader is executing', () => {
  const current = routes.find(route => route.path === 'load-holons')!;
  expect(current.component).toBe(LoadHolonsComponent);
  expect(routes.find(route => route.path === 'load-holons-deprecated')!.component).toBe(DeprecatedLoadHolonsComponent);
  const component = new LoadHolonsComponent();
  const canDismiss = vi.fn(() => false);
  Object.assign(component, { host: { canDismiss } });
  const guard = current.canDeactivate![0] as (component: LoadHolonsComponent) => boolean;
  expect(guard(component)).toBe(false);
  canDismiss.mockReturnValue(true);
  expect(guard(component)).toBe(true);
});
