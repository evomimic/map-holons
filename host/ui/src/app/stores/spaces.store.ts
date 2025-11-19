import { computed, Signal } from '@angular/core';
import { signalStore, withState, withComputed, withMethods, patchState } from '@ngrx/signals';
import { HolonSpace, SpaceType } from '../models/interface.space';

// 1. Define the shape of our state
export interface SpacesState {
  spaces: Record<SpaceType, Record<string, HolonSpace>>;
  loading: boolean;
  error: string | null;
}

// 2. Define the initial state
const initialState: SpacesState = {
  spaces: {
    [SpaceType.Content]: {},
    [SpaceType.Meta]: {},
  },
  loading: false,
  error: null,
};

// 3. Create the SignalStore
export const SpacesStore = signalStore(
  // A. Start with the state shape
  withState(initialState),

  // B. Define computed signals (selectors) to derive data from the state
  withComputed(({ spaces }) => ({
    contentSpaces: computed(() => Object.values(spaces()[SpaceType.Content])),
    metaSpaces: computed(() => Object.values(spaces()[SpaceType.Meta])),
    allSpaces: computed(() => [
      ...Object.values(spaces()[SpaceType.Content]),
      ...Object.values(spaces()[SpaceType.Meta]),
    ]),
    // You can even create computed signals that take parameters
  })),

  // C. Define methods to mutate the state
  withMethods((store) => ({
    addOrUpdateSpace(space: HolonSpace) {
      // Use patchState for immutable, declarative updates
      patchState(store, (state) => ({
        spaces: {
          ...state.spaces,
          [space.space_type]: {
            ...state.spaces[space.space_type],
            [space.id]: space,
          },
        },
      }));
    },
    removeSpace(spaceType: SpaceType, spaceId: string) {
      patchState(store, (state) => {
        const newSpacesForType = { ...state.spaces[spaceType] };
        delete newSpacesForType[spaceId];
        return {
          spaces: {
            ...state.spaces,
            [spaceType]: newSpacesForType,
          },
        };
      });
    },
    getSpaceById(id: string, type?:SpaceType): Signal<HolonSpace | undefined> {
        if (type) {
            return computed(() => store.spaces()[type][id])
        } else {
            return computed(() => store.spaces()[SpaceType.Content][id] || store.spaces()[SpaceType.Meta][id]);
        }
    },
    getHomeSpace(spaceType: SpaceType): HolonSpace | undefined {
    const spaces = Object.values(store.spaces()[spaceType]);
    for (const space of spaces) {
      if (space.id === space.origin_space_id) {
        return space;
      }
    }
    return undefined;
  },
    setLoading(loading: boolean) {
      patchState(store, { loading });
    },
  }))
);