import { Component, effect, OnInit, signal, Signal } from '@angular/core';
import { Router, RouterModule } from '@angular/router';
import { HolonSpace } from '../../models/interface.space';
import { CommonModule } from '@angular/common';
import { SpaceController } from '../../contollers/space.controller';

@Component({
  selector: 'app-content-space',
  standalone: true,
  imports: [CommonModule, RouterModule],
  templateUrl: './content-spaces.html',
})
export class ContentSpace implements OnInit {
  contentSpaces: HolonSpace[] = [];


  constructor(
    private router: Router,
    private spaceController: SpaceController,
  ) { effect(() => {
    console.log('Content Spaces:', this.contentSpaces);
  }); }

  ngOnInit(): void {
      this.contentSpaces = this.spaceController.contentSpaces();

  }

   navigateTo(path: string) {
    this.router.navigate([path]);
  }
}
