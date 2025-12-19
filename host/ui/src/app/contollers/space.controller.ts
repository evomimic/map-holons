import { inject, Injectable, Signal } from '@angular/core';
import { SpacesStore } from '../stores/spaces.store';
import { MultiPlexService } from '../services/multiplex.service';
import { HolonSpace, ProtoAgentSpace, SpaceType } from '../models/interface.space';

@Injectable({
  providedIn: 'root',
})
export class SpaceController {
  private readonly spacesStore = inject(SpacesStore);
  private readonly multiplexService = inject(MultiPlexService);

  // Expose signals from the store for UI components to use
  public readonly allSpaces = this.spacesStore.allSpaces;
  public readonly contentSpaces = this.spacesStore.contentSpaces;
  public readonly metaSpaces = this.spacesStore.metaSpaces;
  public readonly isLoading = this.spacesStore.loading;

  /**
   * Orchestrates the retrieval of a content space by its ID.
   * @param id The ID of the space to retrieve.
   * @returns The space object, or undefined if not found.
   */
  getContentSpaceById(id: string): Signal<HolonSpace | undefined> {
    return this.spacesStore.getSpaceById(id,SpaceType.Content);
  }

    /**
   * Orchestrates the retrieval of a meta space by its ID.
   * @param id The ID of the space to retrieve.
   * @returns The space object, or undefined if not found.
   */
  getMetaSpaceById(id: string): Signal<HolonSpace | undefined> {
    return this.spacesStore.getSpaceById(id,SpaceType.Meta);
  }

  /**
   * Orchestrates the creation of a new space.
   * @param newSpace The new space object to create.
   */
  async createNewSpace(newSpace: ProtoAgentSpace): Promise<void> {
    // The controller's job is to call the service layer.
    //const newSpace: AgentSpace = {
    //  id: Math.random().toString(36).substring(2, 9),
    //  name,
    //  type: this.SPACETYPE,
    //  description: '',
    //  created_at: new Date().toISOString(),
    //  origin_space_id: '',
    //  enabled: true,
    //};
    //let network id = mps.newspace(newspace)
    //this.getStoreById(newSpace.id);
    //return newSpace;
 // }
    await this.multiplexService.createSpace(newSpace);
  }

  /**
   * Orchestrates the deletion of a space.
   * @param space The space object to delete.
   */
  async removeSpace(space: HolonSpace): Promise<void> {
    await this.multiplexService.deleteSpace(space);
  }
}