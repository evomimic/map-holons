import type { BaseValue, PropertyName, RelationshipName } from './types';
import { extractString } from './types';
import type { HolonReference } from './references';

const DESCRIPTOR_HANDLE_CONSTRUCTION = Symbol('DescriptorHandleConstruction');

/** Opaque SDK handle for a holon type descriptor. */
export class HolonDescriptorHandle {
  #reference: HolonReference;

  constructor(
    reference: HolonReference,
    token: typeof DESCRIPTOR_HANDLE_CONSTRUCTION,
  ) {
    if (token !== DESCRIPTOR_HANDLE_CONSTRUCTION) {
      throw new TypeError('HolonDescriptorHandle cannot be constructed directly');
    }

    this.#reference = reference;
  }

  async typeName(): Promise<string> {
    return requiredString(this.#reference.propertyValue('TypeName'), 'TypeName');
  }
}

/** Opaque SDK handle for an effective property descriptor. */
export class PropertyDescriptorHandle {
  #reference: HolonReference;

  constructor(
    reference: HolonReference,
    token: typeof DESCRIPTOR_HANDLE_CONSTRUCTION,
  ) {
    if (token !== DESCRIPTOR_HANDLE_CONSTRUCTION) {
      throw new TypeError('PropertyDescriptorHandle cannot be constructed directly');
    }

    this.#reference = reference;
  }

  async propertyName(): Promise<PropertyName> {
    return requiredString(this.#reference.propertyValue('PropertyName'), 'PropertyName');
  }
}

/** Opaque SDK handle for an effective relationship descriptor. */
export class RelationshipDescriptorHandle {
  #reference: HolonReference;

  constructor(
    reference: HolonReference,
    token: typeof DESCRIPTOR_HANDLE_CONSTRUCTION,
  ) {
    if (token !== DESCRIPTOR_HANDLE_CONSTRUCTION) {
      throw new TypeError('RelationshipDescriptorHandle cannot be constructed directly');
    }

    this.#reference = reference;
  }

  async relationshipName(): Promise<RelationshipName> {
    return requiredString(
      this.#reference.propertyValue('RelationshipName'),
      'RelationshipName',
    );
  }
}

export type RelationshipDirection = 'declared' | 'inverse';

/** A lifecycle-valid relationship descriptor together with its direction. */
export interface AvailableRelationshipHandle {
  readonly descriptor: RelationshipDescriptorHandle;
  readonly direction: RelationshipDirection;
}

export function createHolonDescriptorHandle(
  reference: HolonReference,
): HolonDescriptorHandle {
  return new HolonDescriptorHandle(reference, DESCRIPTOR_HANDLE_CONSTRUCTION);
}

export function createPropertyDescriptorHandle(
  reference: HolonReference,
): PropertyDescriptorHandle {
  return new PropertyDescriptorHandle(reference, DESCRIPTOR_HANDLE_CONSTRUCTION);
}

export function createRelationshipDescriptorHandle(
  reference: HolonReference,
): RelationshipDescriptorHandle {
  return new RelationshipDescriptorHandle(reference, DESCRIPTOR_HANDLE_CONSTRUCTION);
}

async function requiredString(
  value: Promise<BaseValue | null>,
  property: string,
): Promise<string> {
  const resolved = await value;

  if (resolved === null) {
    throw new TypeError(`Descriptor is missing required ${property} property`);
  }

  return extractString(resolved);
}
