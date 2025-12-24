import { Component, effect, inject, OnInit } from '@angular/core';
import { CommonModule } from '@angular/common';
import { Router, RouterModule } from '@angular/router';
import { HolonSpace, ProtoAgentSpace, SpaceType } from '../../models/interface.space';
import { SpaceController } from '../../contollers/space.controller';
import { CreateSpace } from '../new-space/createspace.component';


@Component({
  selector: 'app-main',
  standalone: true,
  imports: [CommonModule, RouterModule, CreateSpace],
  templateUrl: './all-spaces.html',
})
export class AllSpaces implements OnInit {
  private spaceController = inject(SpaceController);
  //private metaController = inject(MetaController);
  
  contentSpaces: HolonSpace[] = [];
  metaSpaces: HolonSpace[] = [];
  
  //allMetaTypes: Signal<Holon[]>;

  showCreateSpaceForm = false;
  derivationOrigin: 'content' | 'meta' | null = null;
  newSpace: ProtoAgentSpace = {
    name: '',
    space_type: SpaceType.Content,
    description: '',
    origin_holon_id: '',
    //metatype_reference: {},
    metadata: {}
  };
  metatypeReferenceJson = '';
  metadataJson = '';
  //selectedMetaType: [string, string] | null = null;


  constructor(
    private router: Router,
  ) {
    //this.allMetaTypes = computed(() => {
    //  const stores = this.metaController.getAllStores();
    //  return stores.flatMap(store => store.committed_holons());
   // });

    effect(() => {
      console.log('Content Spaces:', this.contentSpaces);
      console.log('Meta Spaces:', this.metaSpaces);
      //console.log('All Meta Types:', this.allMetaTypes());
    });
  }

  ngOnInit() {
    this.contentSpaces = this.spaceController.contentSpaces();
    this.metaSpaces = this.spaceController.metaSpaces();
  }

  createSpace(event: { space: ProtoAgentSpace; metadata: string }) {
    try {
      this.newSpace = event.space;
      this.metadataJson = event.metadata;
      /* if (this.selectedMetaType) {
        this.newSpace.metatype_reference = {
          id: this.selectedMetaType[0],
          name: this.selectedMetaType[1]
        };
      } else {
        this.newSpace.metatype_reference = {};
      }*/
      this.newSpace.metadata = this.metadataJson ? JSON.parse(this.metadataJson) : {};
      this.spaceController.createNewSpace(this.newSpace);
    } catch (e) {
      console.error('Error parsing JSON input:', e);
      // Optionally, show an error to the user
    }
  }

  deriveNewSpace(spaceId: string, origin: 'content' | 'meta') {
    this.newSpace.origin_holon_id = spaceId;
    this.derivationOrigin = origin;
    this.showCreateSpaceForm = true;
  }

  cancelCreateSpace() {
    this.showCreateSpaceForm = false;
  }

  navigateTo(path: string) {
    this.router.navigate([path]);
  }
}