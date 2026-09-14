import { Component, inject } from '@angular/core';
import { Router } from '@angular/router';
import { JsonDataUploader } from '../json-data-uploader/json-data-uploader.component';

/**
 * Temporary standalone entry point for canonical LoadHolons verification.
 *
 * It keeps the existing loader UI usable while Canvas is developed, without
 * requiring the retired Content Space browser to discover an upload target.
 */
@Component({
  selector: 'app-load-holons',
  standalone: true,
  imports: [JsonDataUploader],
  template: '<app-json-data-uploader [standaloneMode]="true" (formClosed)="close()"></app-json-data-uploader>',
})
export class LoadHolonsComponent {
  private readonly router = inject(Router);

  close(): void {
    void this.router.navigateByUrl('/');
  }
}
