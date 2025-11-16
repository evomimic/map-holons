import { inject, Injectable, OnDestroy } from "@angular/core";
import { invoke } from '@tauri-apps/api/core';
import { HolonSpace, mockContentSpace, mockMetaSpace, ProtoAgentSpace, SpaceType } from "../models/interface.space";
import { ApiService } from "./api.service";
import { environment } from "../../environments/environment";
import { SpacesStore } from "../stores/spaces.store";
import { MapRequest } from "../models/map.request";
import { map } from "rxjs";

// Helper function to check if the app is running in a Tauri window
const isTauri = () => !!(window as any).__TAURI__;

@Injectable({
  providedIn: "root"
})
export class MultiPlexService extends ApiService implements OnDestroy {

    private readonly spacesStore = inject(SpacesStore);

    async init(): Promise<boolean> {
        if (environment.mock || !isTauri()) {
          console.log("Running in mock mode...");
            sessionStorage.setItem("status", "mock");
            //fake network delay
           // await new Promise(resolve => setTimeout(resolve, 3000));

            this.loadMockSpaces();
            return true;
        }
        sessionStorage.clear();
        // 1. Perform the lightweight readiness check first (non-receptor specific).
        const isReady = await invoke<boolean>('is_service_ready');
        //return await invoke('plugin:holochain|is_holochain_ready');

        // 2. If not ready, return false. The app.config.ts loop will retry.
        if (!isReady) {
            console.log("Backend service not ready yet...");
            return false;
        }
        try {
            console.log("Connecting to the MAP SDK...");
            this.spacesStore.setLoading(true); // Tell the space store we are loading
            const spacesData = await invoke<any>('all_spaces');
            console.log("Spaces JSON received:", spacesData);
            this.processAndStoreSpaces(spacesData);
            return true;
        } catch (error) {
            this.spacesStore.setLoading(false);
            console.error("Error during SDK initialization:", error);
            if (error instanceof Error && error.message.includes("invoke")) {
                console.log("Fallback to mock");
                sessionStorage.setItem("status", "mock:init_error");
                return true
            }
            console.log("service connection error, trying again:");
            return false;
            // Don't block app startup, but indicate error
        }

    }

    /**
     * Parses the raw JSON from the backend, transforms it into AgentSpace objects,
     * and stores them in the inherited space_data record.
     * @param spacesJson The raw JSON string from the `all_spaces` command.
     */
    private processAndStoreSpaces(fetchedData: any): void {
      //const fetchedData = JSON.parse(spacesJson);

      if (!fetchedData || !fetchedData.spaces) {
        console.error("Invalid space data received from backend:", fetchedData);
        return;
      }

      // 2. Get an array of the raw space objects from the backend
      const rawSpaces: any[] = Object.values(fetchedData.spaces);

      // 3. Iterate over each raw space and transform it
      for (const rawSpace of rawSpaces) {
        let mappedType: SpaceType;

        // 4. Map the backend's `space_type` string to your `SpaceType` enum
        switch (rawSpace.space_type) {
          case 'map_holons':
            mappedType = SpaceType.Content;
            break;
          // Add mappings for other types here in the future
          // case 'some_meta_type_from_backend':
          //   mappedType = SpaceType.Meta;
          //   break;
          default:
            // If we don't recognize the type, skip it and log a warning
            console.warn(`Unrecognized space type "${rawSpace.space_type}" from backend. Skipping.`);
            continue;
        }

        // 5. Create the final AgentSpace object with the correct structure
        const agentSpace: HolonSpace = {
          id: rawSpace.id,
          receptor_id: rawSpace.receptor_id,
          name: rawSpace.name,
          space_type: mappedType,
          description: rawSpace.description,
          origin_space_id: rawSpace.origin_space_id, // Map `origin_space` to `origin_space_id`
          enabled: rawSpace.enabled,
          created_at: new Date().toISOString(), // `created_at` is not in backend data, so we can generate it
        };

        // 6. Use the protected `addSpace` method from the parent ApiService
        this.spacesStore.addOrUpdateSpace(agentSpace);
      }

      console.log("Successfully processed and stored space data.");
    }

    async dance(request:MapRequest, timeout=15000): Promise<any>{
      
        console.log(`%c[MULTIPLEX] maprequest. value for invoke:`, 'color: #9C27B0; font-weight: bold;', request);
        //debug:
        //return await invoke("serde_test", {requestJson: JSON.stringify(request)});
        return await invoke("map_request", {mapRequest: request});
        
    }

    loadMockSpaces(){
      this.spacesStore.addOrUpdateSpace(mockContentSpace);
      this.spacesStore.addOrUpdateSpace(mockMetaSpace);
      //this.addSpace(mockContentSpace)
     // this.addSpace(mockMetaSpace)
    };

    /**
     * Calls the backend to create a new space and updates the store on success.
     */
    async createSpace(newSpace: ProtoAgentSpace): Promise<void> {
        try {
            this.spacesStore.setLoading(true);
            const createdSpace = await invoke<HolonSpace>('create_space', newSpace);
            this.spacesStore.addOrUpdateSpace(createdSpace);
        } catch (e) {
            console.error("Failed to create space:", e);
            // Optionally update the store with an error state
        } finally {
            this.spacesStore.setLoading(false);
        }
    }

    /**
     * Calls the backend to delete a space and updates the store on success.
     */
    async deleteSpace(space: HolonSpace): Promise<void> {
        try {
            this.spacesStore.setLoading(true);
            await invoke('delete_space', { id: space.id });
            this.spacesStore.removeSpace(space.space_type, space.id);
        } catch (e) {
            console.error(`Failed to delete space ${space.id}:`, e);
        } finally {
            this.spacesStore.setLoading(false);
        }
    }



    get_space(space_type:string, space_id:string):HolonSpace {
      if (environment.mock) {
        if (space_type == SpaceType.Content) {
          return mockContentSpace; // Replace with actual mock data if needed
        } else if (space_type == SpaceType.Meta) {
          return mockMetaSpace; // Replace with actual mock data if needed
        }
        throw new Error("Mock data not available for space type: " + space_type);
      }
      const space = super.getSpace(space_type,space_id)
      if(!space)
        throw new Error("space for space_type:"+space_type+" with id:"+space_id+" not found");
      return space
    }

    get_home_space(space_type:string): HolonSpace {
      if (environment.mock){
        if (space_type == SpaceType.Content) {
          return mockContentSpace; // Replace with actual mock data if needed
        } else if (space_type == SpaceType.Meta) {
          return mockMetaSpace; // Replace with actual mock data if needed
        }
        throw new Error("Mock data not available for space type: " + space_type);
      }
        const space = super.getHomeSpace(space_type)
        if (!space)
          throw new Error("Home space for type:"+space_type+" not found");
        return space
    }

    get_spaces_by_type(space_type:string):HolonSpace[]{
      //if (environment.mock) {
       // const cells:Cell[] = [mockProvisionedCell,mockClonedCell]
       // return cells
     // }
      return super.getSpacesByType(space_type)
    }


    getConnectionState():string{
      return sessionStorage.getItem("status") || "unknown";
    }

    ngOnDestroy(): void {
        // Cleanup logic if needed
        console.log("SDKService is being destroyed.");      
    }

}