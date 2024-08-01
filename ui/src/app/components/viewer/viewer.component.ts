import { Component, EventEmitter, inject, Inject, OnInit, Output } from '@angular/core';
import { Observable } from 'rxjs'; //it must use the same rxjs as the ngrx package!
import { Holon } from '../../models/holon';
import { HolonStore } from '../../stores/holons.store';
import { HolonsReceptor } from '../../receptors/holons.receptor';
import { CommonModule } from '@angular/common';
import { ClickOutsideDirective } from '../../helpers/clickout';

@Component({
  selector: 'app-viewer',
  standalone: true,
  imports: [CommonModule,ClickOutsideDirective],
  templateUrl: './viewer.component.html',
})
export class ViewerComponent implements OnInit {
  private message_upstream?:string
  @Output() error_message = new EventEmitter()
  public stores:any //HolonStore[]

  constructor(private holon_receptor:HolonsReceptor) {
    try{
    this.stores = holon_receptor.getAllStores()
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
    this.holon_receptor.ngOnDestroy()
  }
}
