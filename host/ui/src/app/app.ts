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
  imports: [CanvasHostComponent, HolonsLoaderHostComponent],
  templateUrl: './app.html',
})
export class App implements OnInit {
  private readonly applicationSession = inject(ApplicationSessionService);
  protected readonly experience = signal<ApplicationExperience | null>(null);
  protected readonly failure = signal<string | null>(null);

  async ngOnInit(): Promise<void> {
    try {
      const session = await this.applicationSession.waitForReady((snapshot) => {
        updateStartupOverlayPhase(snapshot.phase, snapshot.failure);
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
