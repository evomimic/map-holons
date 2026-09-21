import type { BaseValue, PropertyName, RelationshipName } from './types';
import { extractString } from './types';
import { readDescriptorCardinality, readDescriptorIsArray, type HolonReference } from './references';
import { CorePropertyName, CoreRelationshipName } from './core-names';

const DESCRIPTOR_HANDLE_CONSTRUCTION = Symbol('DescriptorHandleConstruction');
const propertyDescriptorReferences = new WeakMap<PropertyDescriptorHandle, HolonReference>();

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
    return requiredString(
      this.#reference.propertyValue(CorePropertyName.TypeName),
      CorePropertyName.TypeName,
    );
  }
}

/** Opaque SDK handle for an effective property descriptor. */
export class PropertyDescriptorHandle {
  constructor(
    reference: HolonReference,
    token: typeof DESCRIPTOR_HANDLE_CONSTRUCTION,
  ) {
    if (token !== DESCRIPTOR_HANDLE_CONSTRUCTION) {
      throw new TypeError('PropertyDescriptorHandle cannot be constructed directly');
    }

    propertyDescriptorReferences.set(this, reference);
  }

  async propertyName(): Promise<PropertyName> {
    return requiredString(
      propertyDescriptorReference(this).propertyValue(CorePropertyName.TypeName),
      CorePropertyName.TypeName,
    );
  }

  /** Rust-owned classification of the declared property representation. */
  isArray(): Promise<boolean> {
    return readDescriptorIsArray(propertyDescriptorReference(this));
  }

  /** Returns the one declared ValueType governing this property descriptor. */
  async valueType(): Promise<ValueDescriptorHandle> {
    const valueTypes = await propertyDescriptorReference(this).relatedHolons(
      CoreRelationshipName.ValueType,
    );
    if (valueTypes.length !== 1) {
      throw new TypeError(
        `PropertyDescriptor must have exactly one ValueType; found ${valueTypes.length}`,
      );
    }
    return createValueDescriptorHandle(valueTypes.members[0]);
  }
}

/** Opaque SDK handle for a declared ValueType descriptor. */
export class ValueDescriptorHandle {
  #reference: HolonReference;

  constructor(
    reference: HolonReference,
    token: typeof DESCRIPTOR_HANDLE_CONSTRUCTION,
  ) {
    if (token !== DESCRIPTOR_HANDLE_CONSTRUCTION) {
      throw new TypeError('ValueDescriptorHandle cannot be constructed directly');
    }
    this.#reference = reference;
  }

  async typeName(): Promise<string> {
    return requiredString(
      this.#reference.propertyValue(CorePropertyName.TypeName),
      CorePropertyName.TypeName,
    );
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

  /** Inclusive bounds resolved from all effective directional constraints. */
  effectiveCardinality(): Promise<EffectiveCardinality> {
    return readDescriptorCardinality(this.#reference);
  }

  /** User-facing relationship label, distinct from its binding name. */
  async displayName(): Promise<string> {
    return requiredString(this.#reference.propertyValue(CorePropertyName.DisplayName), CorePropertyName.DisplayName);
  }

  async relationshipName(): Promise<RelationshipName> {
    return requiredString(
      this.#reference.propertyValue(CorePropertyName.TypeName),
      CorePropertyName.TypeName,
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

export function createValueDescriptorHandle(reference: HolonReference): ValueDescriptorHandle {
  return new ValueDescriptorHandle(reference, DESCRIPTOR_HANDLE_CONSTRUCTION);
}

/**
 * Internal transport projection for a descriptor handle already bound to one
 * transaction. Public callers use MapTransaction's descriptor-aware selection
 * methods rather than treating descriptor identity as an ordinary holon.
 */
export function unwrapPropertyDescriptorHandle(
  handle: PropertyDescriptorHandle,
): HolonReference {
  return propertyDescriptorReference(handle);
}

function propertyDescriptorReference(handle: PropertyDescriptorHandle): HolonReference {
  const reference = propertyDescriptorReferences.get(handle);
  if (reference === undefined) {
    throw new TypeError('PropertyDescriptorHandle is not bound to a reference');
  }
  return reference;
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

/** Inclusive effective bounds; null maximum means unbounded. */
export interface EffectiveCardinality {
  readonly minimum: number;
  readonly maximum: number | null;
}
