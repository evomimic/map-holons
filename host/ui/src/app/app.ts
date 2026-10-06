import { Router, RouterOutlet, NavigationEnd } from '@angular/router';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { Component, OnInit, inject, signal } from '@angular/core';
import { CanvasHostComponent } from './components/canvas-host/canvas-host.component';
import { HolonsLoaderHostComponent } from './components/holons-loader-host/holons-loader-host.component';
import {
  ApplicationSessionService,
  type ApplicationExperience,
} from './services/application-session.service';
import { updateStartupOverlayPhase } from './startup-overlay';

@Component({
  selector: 'app-root',
  standalone: true,
  imports: [CanvasHostComponent, HolonsLoaderHostComponent, RouterOutlet],
  templateUrl: './app.html',
})
export class App implements OnInit {
  private readonly router = inject(Router);
  protected readonly loaderRoute = signal(false);
  protected readonly deprecatedLoaderRoute = signal(false);
  constructor() {
    const update = (url: string) => {
      this.loaderRoute.set(/^\/load-holons(?:[?#]|$)/.test(url));
      this.deprecatedLoaderRoute.set(/^\/load-holons-deprecated(?:[?#]|$)/.test(url));
    };
    update(this.router.url);
    this.router.events.pipe(takeUntilDestroyed()).subscribe(event => {
      if (event instanceof NavigationEnd) update(event.urlAfterRedirects);
    });
  }
  private readonly applicationSession = inject(ApplicationSessionService);
  protected readonly experience = signal<ApplicationExperience | null>(null);
  protected readonly failure = signal<string | null>(null);

  async ngOnInit(): Promise<void> {
    try {
      const session = await this.applicationSession.waitForReady((snapshot) => {
        updateStartupOverlayPhase(snapshot.phase, snapshot.failure, snapshot.dev_mode);
      });
      if (session.phase !== 'ready') {
        this.failure.set(session.failure ?? `Application session stopped in '${session.phase}'.`);
        return;
      }
      this.experience.set(session.experience);
    } catch (error) {
      this.failure.set(error instanceof Error ? error.message : String(error));
    }
  }
}
