import { Component, effect, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterOutlet } from '@angular/router';

import { HolonTypeStore } from './stores/holontypes.store';
import { TypesReceptor } from './receptors/types.receptor';
import { getState } from '@ngrx/signals';
import { ToolbarComponent } from './components/toolbar/toolbar.component';
import { FooterComponent } from './components/footer/footer.component';
import { ViewerComponent } from './components/viewer/viewer.component';

@Component({
  selector: 'app-root',
  standalone: true,
  imports: [RouterOutlet,
    CommonModule,
    ToolbarComponent,
    FooterComponent,
    ViewerComponent
    ],
  templateUrl: './app.component.html',
  //providers:[{
   // provide:ProfileStore,
    //useFactory: (receptor:MyReceptor) => {return receptor.getStore("profile")},
    //deps:[MyReceptor]
  //}],
})
export class AppComponent {
  title = 'my-app';
  //readonly store = inject(HolonTypeStore)
  status:string | null = ""
  statusStyling:string = "text-green-500"

  constructor(){
    effect(() => {
      // 👇 The effect will be re-executed whenever the state changes.
      //const state = getState(this.store);
      //console.log('profile state changed', state);
    });
  }
}
