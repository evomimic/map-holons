import { Component, EventEmitter, Inject, OnDestroy, OnInit, Output, effect, inject } from '@angular/core';
import { HolonTypeStore } from '../../stores/holontypes.store';
import { getState } from '@ngrx/signals';
import { CommonModule } from '@angular/common';
import { TypesReceptor } from '../../receptors/types.receptor';
import { SignalStore } from '../../helpers/interface.store';
import { ClickOutsideDirective } from '../../helpers/clickout';


@Component({
  selector: 'app-toolbar',
  standalone: true,
  imports: [CommonModule, ClickOutsideDirective],
  templateUrl: './toolbar.component.html',
})
export class ToolbarComponent implements OnDestroy {
  private message_upstream?:string
  @Output() error_message = new EventEmitter()
  public openTypeList:boolean = false
  public showHolonList:boolean = false
  public store: any //= inject(HolonTypeStore) 
  
  constructor(private typereceptor:TypesReceptor) {
    try {
      this.store = this.typereceptor.getStore()
      console.log(this.store)
      effect(() => {
        // 👇 The effect will be re-executed whenever the state changes.
        const state = getState(this.store);
        console.log('store state changed', state);
      });
    } catch(err:any) {
      console.error(err)
      this.message_upstream = err
    }
  }

  async ngOnInit(): Promise<void> {
    if (this.message_upstream)
      this.error_message.emit(this.message_upstream)
  }

  toggleMenu(){
    if(this.openTypeList){
      this.openTypeList = false
      this.showHolonList = false
    }else {
      this.showHolonList = false
      this.openTypeList = true
    }
  }

  openHolonList(){
      this.openTypeList = false
      this.showHolonList = true
  }

  closeHolonList(){
    this.openTypeList = false
    this.showHolonList = false
  }

  ngOnDestroy(): void {
    //this.store = null
    this.typereceptor.ngOnDestroy()
  }
};
