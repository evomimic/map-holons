import { Injectable, Injector, Signal } from '@angular/core';
import { SignalStore } from '../models/interface.store';

type StoreFactory<T> = (injector: Injector, id: string) => T;

@Injectable({
  providedIn: 'root',
})
export class StoreManager {
  private storeCache = new Map<string, SignalStore>();

  constructor(private injector: Injector) {}

  getOrCreate<T extends SignalStore>(key: string, factory: StoreFactory<T>): T {
    if (!this.storeCache.has(key)) {
      const newStore = factory(this.injector, key);
      this.storeCache.set(key, newStore);
    }
    return this.storeCache.get(key) as T;
  }

  /**
   * Removes a store instance from the cache, allowing it to be garbage collected.
   * @param key The unique key of the store to destroy.
   */
  public destroy(key: string): void {
    if (this.storeCache.has(key)) {
      // Optional: If the store instance itself has an ngOnDestroy method, call it.
      const storeInstance = this.storeCache.get(key);
      if (storeInstance && typeof storeInstance.onDestroy === 'function') {
        storeInstance.onDestroy();
      }
      
      this.storeCache.delete(key);
      console.log(`[StoreManager] Destroyed and removed store: ${key}`);
    }
  }
}