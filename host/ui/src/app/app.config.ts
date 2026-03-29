import { ApplicationConfig, inject, provideAppInitializer, provideBrowserGlobalErrorListeners } from '@angular/core';
import { provideRouter } from '@angular/router';
import { MultiPlexService } from './services/multiplex.service';
import { routes } from './app.routes';
import { SpacesStore } from './stores/spaces.store';

export const appConfig: ApplicationConfig = {
  providers: [
    provideBrowserGlobalErrorListeners(),
   // provideZonelessChangeDetection(),
    provideRouter(routes),
    SpacesStore,
    provideAppInitializer(async () => {
      const mpService = inject(MultiPlexService);
      await mpService.waitForStartupReady();
      await mpService.init();
      return true;
    })
  ]
};
