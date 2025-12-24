import { Component, EventEmitter, inject, Inject, OnInit, Output } from '@angular/core';
import { Observable } from 'rxjs'; //it must use the same rxjs as the ngrx package!
import { Holon } from '../../models/holon';
//import { ContentStore } from '../../stores/content.store';
import { ContentController } from '../../contollers/content.controller';
import { CommonModule } from '@angular/common';
import { ClickOutsideDirective } from '../../helpers/clickout';

@Component({
  selector: 'app-viewer',
  standalone: true,
  imports: [CommonModule],//,ClickOutsideDirective],
  templateUrl: './viewer.component.html',
  providers: [ContentController],
})
export class ViewerComponent implements OnInit {
  private message_upstream?:string
  @Output() error_message = new EventEmitter()
  public stores:any //HolonStore[]

  constructor(private content_controller:ContentController) {
    try{
    this.stores = content_controller.getAllStores()
    console.log(this.stores[0])//.last_dance_response())
    } catch(err:any){
      console.log(err)
      this.message_upstream = err

    }
  }
  
  ngOnInit(): void {
  }

  ngOnDestroy(): void {
    //this.store = null
    this.content_controller.ngOnDestroy()
  }
}
