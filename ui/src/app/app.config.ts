import { APP_INITIALIZER, ApplicationConfig, provideExperimentalZonelessChangeDetection } from '@angular/core';
import { provideRouter, withComponentInputBinding } from '@angular/router';
import { HolochainService } from './services/holochain.service';

import { routes } from './app.routes';

export function initializeConnection(holochainService: HolochainService) {
  return (): Promise<any> => { 
    return holochainService.init();
  }
}

export const appConfig: ApplicationConfig = {
  providers: [provideRouter(routes,withComponentInputBinding()), 
              provideExperimentalZonelessChangeDetection(),
              { provide: APP_INITIALIZER, useFactory: initializeConnection, deps: [HolochainService], multi: true}]
};
//provideZoneChangeDetection({ eventCoalescing: true }),