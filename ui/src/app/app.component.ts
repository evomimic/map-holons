import { Component, effect } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterOutlet } from '@angular/router';
import { ToolbarComponent } from './components/toolbar/toolbar.component';
import { FooterComponent } from './components/footer/footer.component';
import { ViewerComponent } from './components/viewer/viewer.component';

@Component({
  selector: 'app-root',
  standalone: true,
  imports: [//RouterOutlet,
    CommonModule,
    ToolbarComponent,
    FooterComponent,
    ViewerComponent
    ],
  templateUrl: './app.component.html',

})
export class AppComponent {
  title = 'my-app';
  error:string | null = ""
  errorStyling:string = "text-red-500"
  status:string | null = ""
  statusStyling:string = "text-green-500"

  constructor(){
    //effect(() => {
      // 👇 The effect will be re-executed whenever the state changes.
      //const state = getState(this.store);
      //console.log('profile state changed', state);
    //});
  }

  errorDownstream(message:string){
    console.log(message)
    this.error = message
  }
}
