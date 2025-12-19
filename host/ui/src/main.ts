import { bootstrapApplication } from '@angular/platform-browser';
import { appConfig } from './app/app.config';
import { App } from './app/app';
import { environment } from './environments/environment';

bootstrapApplication(App, appConfig)
  .then(() => {
    //if (environment.mock) {
    //  setTimeout(() => {
        const loadingOverlay = document.getElementById('loading-overlay');
        if (loadingOverlay) {
          loadingOverlay.classList.add('opacity-0', 'pointer-events-none');
        }
    //  }, 4000);
    //} else {
      //const loadingOverlay = document.getElementById('loading-overlay');
      //if (loadingOverlay) {
      //  loadingOverlay.classList.add('hidden');
     // }
   // }
  })
  .catch((err) => console.error(err));
