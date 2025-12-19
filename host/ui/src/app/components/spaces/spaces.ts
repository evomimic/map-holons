import { Component, computed, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { ActivatedRoute, Router, RouterModule } from '@angular/router';
import { FormsModule } from '@angular/forms';
import { HolonSpace, ProtoAgentSpace, SpaceType } from '../../models/interface.space';
import { SpaceController } from '../../contollers/space.controller';

@Component({
  selector: 'app-spaces',
  standalone: true,
  imports: [CommonModule, RouterModule, FormsModule],
  templateUrl: './spaces.html',
})
export class Spaces {
  private spaceController = inject(SpaceController);
  private route = inject(ActivatedRoute);
  private router = inject(Router);

  // 1. Get the raw signals for all spaces from the controller.
  private readonly allContentSpaces = this.spaceController.contentSpaces;
  private readonly allMetaSpaces = this.spaceController.metaSpaces;

  // 2. Create a signal to hold the current filter type from the route.
  private spaceTypeToShow = signal<'all' | 'content' | 'meta'>('all');

  // 3. Create computed signals that reactively filter the spaces.
  public readonly contentSpaces = computed(() => {
    const filter = this.spaceTypeToShow();
    return (filter === 'all' || filter === 'content') ? this.allContentSpaces() : [];
  });

  public readonly metaSpaces = computed(() => {
    const filter = this.spaceTypeToShow();
    return (filter === 'all' || filter === 'meta') ? this.allMetaSpaces() : [];
  });

  // Form state
  showCreateSpaceForm = false;
  derivationOrigin = signal<'content' | 'meta' | null>(null);
  newSpace: ProtoAgentSpace = {
    name: '',
    space_type: SpaceType.Content,
    description: '',
    origin_holon_id: '',
    metadata: {}
  };
  metadataJson = '';

  constructor() {
    // 4. Subscribe to route data changes to update the filter signal.
    // We check `firstChild` because the data is on the child routes we defined.
    this.route.firstChild?.data.subscribe(data => {
      if (data['filter']) {
        this.spaceTypeToShow.set(data['filter']);
      }
    });
  }

  createSpace() {
    try {
      this.newSpace.metadata = this.metadataJson ? JSON.parse(this.metadataJson) : {};
      this.spaceController.createNewSpace(this.newSpace);
      this.showCreateSpaceForm = false; // Hide form after creation
    } catch (e) {
      console.error('Error parsing JSON input:', e);
    }
  }

  deriveNewSpace(spaceId: string, origin: 'content' | 'meta') {
    this.newSpace.origin_holon_id = spaceId;
    this.derivationOrigin.set(origin);
    this.showCreateSpaceForm = true;
  }

  cancelCreateSpace() {
    this.showCreateSpaceForm = false;
  }
}