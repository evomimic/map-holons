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
      let result = false;
      let attempts = 0;
      while (!result) {
        try {
          result = await mpService.init();
        } catch (error) {
          console.error('Error initializing SDK, will retry:', error);
        }
        if (!result) {
          attempts++;
          const delay = Math.min(1000 * 1.5 ** attempts, 30000);
          console.log(`SDK initialization failed. Retrying in ${delay}ms...`);
          await new Promise(resolve => setTimeout(resolve, delay));
        }
      }
      return true;
    })
  ]
};
