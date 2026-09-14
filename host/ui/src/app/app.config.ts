import { ApplicationConfig, provideBrowserGlobalErrorListeners } from '@angular/core';
import { provideRouter } from '@angular/router';
import { routes } from './app.routes';
import { SpacesStore } from './stores/spaces.store';

export const appConfig: ApplicationConfig = {
  providers: [
    provideBrowserGlobalErrorListeners(),
    provideRouter(routes),
    // The temporary Holons Loader experience retains its existing store while
    // Canvas remains independent of loader-era navigation state.
    SpacesStore,
  ]
};
