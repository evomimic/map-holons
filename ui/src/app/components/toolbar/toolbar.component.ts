import { Component, Inject, OnDestroy, OnInit, effect, inject } from '@angular/core';
import { HolonTypeStore } from '../../stores/holontypes.store';
import { getState } from '@ngrx/signals';
import { CommonModule } from '@angular/common';
import { TypesReceptor } from '../../receptors/types.receptor';


@Component({
  selector: 'app-toolbar',
  standalone: true,
  imports: [CommonModule],
  templateUrl: './toolbar.component.html',
})
export class ToolbarComponent implements OnDestroy {
  public openTypeList:boolean = false
  public showHolonList:boolean = false
  readonly store: HolonTypeStore// = inject(HolonTypeStore)
  
  constructor(private receptor:TypesReceptor) {
    this.store = receptor.getStore("holontype_store","holontypes")
    effect(() => {
      // 👇 The effect will be re-executed whenever the state changes.
      const state = getState(this.store);
      console.log('store state changed', state);
    });
  }

  ngOnInit(): void {
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
    this.receptor.ngOnDestroy()
  }
}
