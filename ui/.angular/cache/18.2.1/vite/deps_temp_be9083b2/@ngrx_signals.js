import {
  DestroyRef,
  Injectable,
  Injector,
  SIGNAL,
  assertInInjectionContext,
  computed,
  inject,
  isSignal,
  setClassMetadata,
  signal,
  untracked,
  ɵɵdefineInjectable
} from "./chunk-E2QOHRC5.js";
import {
  __spreadProps,
  __spreadValues
} from "./chunk-46DXP6YY.js";

// ../node_modules/@ngrx/signals/fesm2022/ngrx-signals.mjs
var STATE_WATCHERS = /* @__PURE__ */ new WeakMap();
var STATE_SOURCE = Symbol("STATE_SOURCE");
function patchState(stateSource, ...updaters) {
  stateSource[STATE_SOURCE].update((currentState) => updaters.reduce((nextState, updater) => __spreadValues(__spreadValues({}, nextState), typeof updater === "function" ? updater(nextState) : updater), currentState));
  notifyWatchers(stateSource);
}
function getState(stateSource) {
  return stateSource[STATE_SOURCE]();
}
function watchState(stateSource, watcher, config) {
  if (!config?.injector) {
    assertInInjectionContext(watchState);
  }
  const injector = config?.injector ?? inject(Injector);
  const destroyRef = injector.get(DestroyRef);
  addWatcher(stateSource, watcher);
  watcher(getState(stateSource));
  const destroy = () => removeWatcher(stateSource, watcher);
  destroyRef.onDestroy(destroy);
  return {
    destroy
  };
}
function getWatchers(stateSource) {
  return STATE_WATCHERS.get(stateSource[STATE_SOURCE][SIGNAL]) || [];
}
function notifyWatchers(stateSource) {
  const watchers = getWatchers(stateSource);
  for (const watcher of watchers) {
    const state = untracked(() => getState(stateSource));
    watcher(state);
  }
}
function addWatcher(stateSource, watcher) {
  const watchers = getWatchers(stateSource);
  STATE_WATCHERS.set(stateSource[STATE_SOURCE][SIGNAL], [...watchers, watcher]);
}
function removeWatcher(stateSource, watcher) {
  const watchers = getWatchers(stateSource);
  STATE_WATCHERS.set(stateSource[STATE_SOURCE][SIGNAL], watchers.filter((w) => w !== watcher));
}
function toDeepSignal(signal2) {
  const value = untracked(() => signal2());
  if (!isRecord(value)) {
    return signal2;
  }
  return new Proxy(signal2, {
    get(target, prop) {
      if (!(prop in value)) {
        return target[prop];
      }
      if (!isSignal(target[prop])) {
        Object.defineProperty(target, prop, {
          value: computed(() => target()[prop]),
          configurable: true
        });
      }
      return toDeepSignal(target[prop]);
    }
  });
}
function isRecord(value) {
  return value?.constructor === Object;
}
function signalState(initialState) {
  const stateSource = signal(initialState);
  const signalState2 = toDeepSignal(stateSource.asReadonly());
  Object.defineProperty(signalState2, STATE_SOURCE, {
    value: stateSource
  });
  return signalState2;
}
function signalStore(...args) {
  const signalStoreArgs = [...args];
  const config = typeof signalStoreArgs[0] === "function" ? {} : signalStoreArgs.shift();
  const features = signalStoreArgs;
  class SignalStore {
    constructor() {
      const innerStore = features.reduce((store, feature) => feature(store), getInitialInnerStore());
      const {
        stateSignals,
        computedSignals,
        methods,
        hooks
      } = innerStore;
      const storeMembers = __spreadValues(__spreadValues(__spreadValues({}, stateSignals), computedSignals), methods);
      this[STATE_SOURCE] = config.protectedState === false ? innerStore[STATE_SOURCE] : innerStore[STATE_SOURCE].asReadonly();
      for (const key in storeMembers) {
        this[key] = storeMembers[key];
      }
      const {
        onInit,
        onDestroy
      } = hooks;
      if (onInit) {
        onInit();
      }
      if (onDestroy) {
        inject(DestroyRef).onDestroy(onDestroy);
      }
    }
    /** @nocollapse */
    static ɵfac = function SignalStore_Factory(__ngFactoryType__) {
      return new (__ngFactoryType__ || SignalStore)();
    };
    /** @nocollapse */
    static ɵprov = ɵɵdefineInjectable({
      token: SignalStore,
      factory: SignalStore.ɵfac,
      providedIn: config.providedIn || null
    });
  }
  (() => {
    (typeof ngDevMode === "undefined" || ngDevMode) && setClassMetadata(SignalStore, [{
      type: Injectable,
      args: [{
        providedIn: config.providedIn || null
      }]
    }], () => [], null);
  })();
  return SignalStore;
}
function getInitialInnerStore() {
  return {
    [STATE_SOURCE]: signal({}),
    stateSignals: {},
    computedSignals: {},
    methods: {},
    hooks: {}
  };
}
function signalStoreFeature(featureOrInput, ...restFeatures) {
  const features = typeof featureOrInput === "function" ? [featureOrInput, ...restFeatures] : restFeatures;
  return (inputStore) => features.reduce((store, feature) => feature(store), inputStore);
}
function type() {
  return void 0;
}
function assertUniqueStoreMembers(store, newMemberKeys) {
  if (!ngDevMode) {
    return;
  }
  const storeMembers = __spreadValues(__spreadValues(__spreadValues({}, store.stateSignals), store.computedSignals), store.methods);
  const overriddenKeys = Object.keys(storeMembers).filter((memberKey) => newMemberKeys.includes(memberKey));
  if (overriddenKeys.length > 0) {
    console.warn("@ngrx/signals: SignalStore members cannot be overridden.", "Trying to override:", overriddenKeys.join(", "));
  }
}
function withComputed(signalsFactory) {
  return (store) => {
    const computedSignals = signalsFactory(__spreadValues(__spreadValues({}, store.stateSignals), store.computedSignals));
    assertUniqueStoreMembers(store, Object.keys(computedSignals));
    return __spreadProps(__spreadValues({}, store), {
      computedSignals: __spreadValues(__spreadValues({}, store.computedSignals), computedSignals)
    });
  };
}
function withHooks(hooksOrFactory) {
  return (store) => {
    const storeMembers = __spreadValues(__spreadValues(__spreadValues({
      [STATE_SOURCE]: store[STATE_SOURCE]
    }, store.stateSignals), store.computedSignals), store.methods);
    const hooks = typeof hooksOrFactory === "function" ? hooksOrFactory(storeMembers) : hooksOrFactory;
    const createHook = (name) => {
      const hook = hooks[name];
      const currentHook = store.hooks[name];
      return hook ? () => {
        if (currentHook) {
          currentHook();
        }
        hook(storeMembers);
      } : currentHook;
    };
    return __spreadProps(__spreadValues({}, store), {
      hooks: {
        onInit: createHook("onInit"),
        onDestroy: createHook("onDestroy")
      }
    });
  };
}
function withMethods(methodsFactory) {
  return (store) => {
    const methods = methodsFactory(__spreadValues(__spreadValues(__spreadValues({
      [STATE_SOURCE]: store[STATE_SOURCE]
    }, store.stateSignals), store.computedSignals), store.methods));
    assertUniqueStoreMembers(store, Object.keys(methods));
    return __spreadProps(__spreadValues({}, store), {
      methods: __spreadValues(__spreadValues({}, store.methods), methods)
    });
  };
}
function withState(stateOrFactory) {
  return (store) => {
    const state = typeof stateOrFactory === "function" ? stateOrFactory() : stateOrFactory;
    const stateKeys = Object.keys(state);
    assertUniqueStoreMembers(store, stateKeys);
    store[STATE_SOURCE].update((currentState) => __spreadValues(__spreadValues({}, currentState), state));
    const stateSignals = stateKeys.reduce((acc, key) => {
      const sliceSignal = computed(() => store[STATE_SOURCE]()[key]);
      return __spreadProps(__spreadValues({}, acc), {
        [key]: toDeepSignal(sliceSignal)
      });
    }, {});
    return __spreadProps(__spreadValues({}, store), {
      stateSignals: __spreadValues(__spreadValues({}, store.stateSignals), stateSignals)
    });
  };
}
export {
  getState,
  patchState,
  signalState,
  signalStore,
  signalStoreFeature,
  type,
  watchState,
  withComputed,
  withHooks,
  withMethods,
  withState
};
//# sourceMappingURL=@ngrx_signals.js.map
