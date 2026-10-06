import { Injectable, inject } from '@angular/core';
import type { CanActivateFn } from '@angular/router';

/** Protect Canvas owners mounted outside the router outlet as well as routed ones. */
@Injectable({ providedIn: 'root' })
export class CanvasNavigationGuard {
  private readonly owners = new Set<{ canDismiss(): boolean }>();

  register(owner: { canDismiss(): boolean }): () => void {
    this.owners.add(owner);
    return () => { this.owners.delete(owner); };
  }

  canNavigate(): boolean {
    return [...this.owners].every(owner => owner.canDismiss());
  }
}

export const canNavigateFromCanvas: CanActivateFn = () => inject(CanvasNavigationGuard).canNavigate();
