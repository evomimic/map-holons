import { Component, ViewChild } from '@angular/core';
import { CanvasHostComponent } from '../canvas-host/canvas-host.component';

/** Route adapter; the Canvas resolves and retains the session's active Space. */
@Component({
  selector: 'app-load-holons',
  standalone: true,
  imports: [CanvasHostComponent],
  template: '<app-canvas-host [launchLoadHolons]="true"></app-canvas-host>',
})
export class LoadHolonsComponent {
  @ViewChild(CanvasHostComponent) private host?: CanvasHostComponent;
  canDismiss(): boolean { return this.host?.canDismiss() ?? true; }
}
