import type { BaseValue, PropertyName, RelationshipName } from './types';
import { extractString } from './types';
import { constructResolvedDescriptor, readInstanceProperties, readPropertyValueKind, readDescriptorCardinality, readDescriptorIsArray, readDescriptorIsOrdered, readDescriptorHasInstanceKey, type HolonReference } from './references';
import { CorePropertyName, CoreRelationshipName } from './core-names';

/** Contribution Holons are opt-in; effective products always retain declaration occurrences. */
export interface EffectiveDescriptorOptions { readonly includeContributions?: boolean; }

const DESCRIPTOR_HANDLE_CONSTRUCTION = Symbol('DescriptorHandleConstruction');
const holonDescriptorReferences = new WeakMap<HolonDescriptorHandle, HolonReference>();
const propertyDescriptorReferences = new WeakMap<PropertyDescriptorHandle, HolonReference>();

/** Opaque SDK handle for a holon type descriptor. */
export class HolonDescriptorHandle {
  /** Constructs a new resolved schema snapshot in this handle's retained transaction. */
  effectiveDescriptor(options: EffectiveDescriptorOptions = {}): Promise<EffectiveDescriptorHandle> { return constructResolvedDescriptor(this.#reference, options); }

  /** Whether the effective instance key rule defines keys, resolved by Rust. */
  hasInstanceKey(): Promise<boolean> { return readDescriptorHasInstanceKey(this.#reference); }

  /** Effective instance properties, in runtime-provided order. */
  instanceProperties(): Promise<ReadonlyArray<PropertyDescriptorHandle>> { return readInstanceProperties(this.#reference); }
  #reference: HolonReference;

  constructor(
    reference: HolonReference,
    token: typeof DESCRIPTOR_HANDLE_CONSTRUCTION,
  ) {
    if (token !== DESCRIPTOR_HANDLE_CONSTRUCTION) {
      throw new TypeError('HolonDescriptorHandle cannot be constructed directly');
    }

    this.#reference = reference;
    holonDescriptorReferences.set(this, reference);
  }

  /** User-facing name of this HolonType. */
  displayName(): Promise<string> {
    return requiredString(this.#reference.propertyValue(CorePropertyName.DisplayName), CorePropertyName.DisplayName);
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

  /** Display heading is separate from the property's binding name. */
  displayName(): Promise<string> {
    return requiredString(propertyDescriptorReference(this).propertyValue(CorePropertyName.DisplayName), CorePropertyName.DisplayName);
  }

  /** Rust-owned scalar kind; AnyBaseValue permits descriptor-authorized mixed cells. */
  async valueKind(): Promise<ScalarValueKind> {
    const kind = await readPropertyValueKind(propertyDescriptorReference(this));
    if (!['StringValue', 'BooleanValue', 'IntegerValue', 'EnumValue', 'BytesValue', 'AnyBaseValue'].includes(kind)) throw new TypeError(`Unsupported property representation: ${kind}`);
    return kind as ScalarValueKind;
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

  /** Whether related members have schema-significant order, resolved by Rust. */
  isOrdered(): Promise<boolean> {
    return readDescriptorIsOrdered(this.#reference);
  }

  /** Inclusive bounds resolved from all effective directional constraints. */
  effectiveCardinality(): Promise<EffectiveCardinality> {
    return readDescriptorCardinality(this.#reference);
  }

  /** User-facing relationship label, distinct from its binding name. */
  async displayName(): Promise<string> {
    return requiredString(this.#reference.propertyValue(CorePropertyName.DisplayName), CorePropertyName.DisplayName);
  }

  /** Optional descriptive text from this directional relationship descriptor. */
  async description(): Promise<string | null> {
    const value = await this.#reference.propertyValue(CorePropertyName.Description);
    return value === null ? null : extractString(value);
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

export type ScalarValueKind = 'StringValue' | 'BooleanValue' | 'IntegerValue' | 'EnumValue' | 'BytesValue' | 'AnyBaseValue';

/** Internal binding bridge; callers retain the opaque descriptor handle. */
export function unwrapHolonDescriptorHandle(handle: HolonDescriptorHandle): HolonReference {
  const reference = holonDescriptorReferences.get(handle);
  if (!reference) throw new TypeError('Invalid HolonDescriptorHandle');
  return reference;
}

const effectiveDescriptorReferences = new WeakMap<EffectiveDescriptorHandle, HolonReference>();

/** Read-only, reference-backed access to one transaction-scoped resolved schema snapshot. */
export class EffectiveDescriptorHandle {
  #nameIndexes = new Map<RelationshipName, Promise<Map<string, readonly HolonReference[]>>>();

  private named(collection: RelationshipName, name: string): Promise<HolonReference | null> {
    let index = this.#nameIndexes.get(collection);
    if (!index) {
      index = this.members(collection).then(async members => {
        const result = new Map<string, HolonReference[]>();
        for (const member of members) {
          const value = await member.propertyValue(CorePropertyName.TypeName);
          if (value === null) continue;
          const memberName = extractString(value);
          const occurrences = result.get(memberName) ?? [];
          occurrences.push(member);
          result.set(memberName, occurrences);
        }
        return result;
      });
      this.#nameIndexes.set(collection, index);
    }
    return index.then(entries => {
      const matches = entries.get(name) ?? [];
      if (matches.length > 1) throw new TypeError(`Ambiguous ${collection} name '${name}': ${matches.length} declaration occurrences`);
      return matches[0] ?? null;
    });
  }

  async propertyNamed(name: string): Promise<PropertyDescriptorHandle | null> {
    const reference = await this.named('EffectiveInstanceProperties', name);
    return reference === null ? null : createPropertyDescriptorHandle(reference);
  }
  async relationshipNamed(name: string): Promise<RelationshipDescriptorHandle | null> {
    const reference = await this.named('NavigableRelationships', name);
    return reference === null ? null : createRelationshipDescriptorHandle(reference);
  }
  danceNamed(name: string): Promise<HolonReference | null> { return this.named('EffectiveAffordsDance', name); }
  commandNamed(name: string): Promise<HolonReference | null> { return this.named('EffectiveAffordsCommand', name); }

  constructor(reference: HolonReference, token: typeof DESCRIPTOR_HANDLE_CONSTRUCTION) {
    if (token !== DESCRIPTOR_HANDLE_CONSTRUCTION) throw new TypeError('EffectiveDescriptorHandle cannot be constructed directly');
    effectiveDescriptorReferences.set(this, reference);
  }
  private async members(name: RelationshipName): Promise<ReadonlyArray<HolonReference>> {
    return Object.freeze([...(await unwrapEffectiveDescriptorHandle(this).relatedHolons(name)).members]);
  }
  async describedHolonType(): Promise<HolonDescriptorHandle> {
    const members = await this.members('DescribedHolonType');
    if (members.length !== 1) throw new TypeError('Expected exactly one DescribedHolonType');
    return createHolonDescriptorHandle(members[0]);
  }
  async instanceProperties(): Promise<ReadonlyArray<PropertyDescriptorHandle>> {
    return Object.freeze((await this.members('EffectiveInstanceProperties')).map(createPropertyDescriptorHandle));
  }
  async instanceRelationships(): Promise<ReadonlyArray<RelationshipDescriptorHandle>> {
    return Object.freeze((await this.members('EffectiveInstanceRelationships')).map(createRelationshipDescriptorHandle));
  }
  async navigableRelationships(): Promise<ReadonlyArray<RelationshipDescriptorHandle>> {
    return Object.freeze((await this.members('NavigableRelationships')).map(createRelationshipDescriptorHandle));
  }
  affordedDances(): Promise<ReadonlyArray<HolonReference>> { return this.members('EffectiveAffordsDance'); }
  affordedCommands(): Promise<ReadonlyArray<HolonReference>> { return this.members('EffectiveAffordsCommand'); }
  effectiveConstraints(): Promise<ReadonlyArray<HolonReference>> { return this.members('EffectiveConstraints'); }
  effectiveValidationBindings(): Promise<ReadonlyArray<HolonReference>> { return this.members('EffectiveValidationBindings'); }
  async effectiveKeyRule(): Promise<HolonReference> {
    const members = await this.members('EffectiveInstanceKeyRule');
    if (members.length !== 1) throw new TypeError('Expected exactly one EffectiveInstanceKeyRule');
    return members[0];
  }
  async contributions(): Promise<ReadonlyArray<EffectiveSchemaContributionHandle>> {
    return Object.freeze((await this.members('EffectiveContributions')).map(reference => new EffectiveSchemaContributionHandle(reference, DESCRIPTOR_HANDLE_CONSTRUCTION)));
  }
}

/** A provenance record; collection, member and origin remain ordinary graph references. */
export class EffectiveSchemaContributionHandle {
  #reference: HolonReference;
  constructor(reference: HolonReference, token: typeof DESCRIPTOR_HANDLE_CONSTRUCTION) {
    if (token !== DESCRIPTOR_HANDLE_CONSTRUCTION) throw new TypeError('EffectiveSchemaContributionHandle cannot be constructed directly');
    this.#reference = reference;
  }
  private async one(name: RelationshipName): Promise<HolonReference> {
    const members = (await this.#reference.relatedHolons(name)).members;
    if (members.length !== 1) throw new TypeError(`Expected exactly one ${name}`);
    return members[0];
  }
  collection(): Promise<HolonReference> { return this.one('ContributionCollection'); }
  member(): Promise<HolonReference> { return this.one('ContributionMember'); }
  declaredOn(): Promise<HolonReference> { return this.one('ContributionDeclaredOn'); }
  async position(): Promise<number> {
    const value = await this.#reference.propertyValue('ContributionPosition');
    if (!value || !('IntegerValue' in value) || !Number.isSafeInteger(value.IntegerValue) || value.IntegerValue < 0) throw new TypeError('Invalid ContributionPosition');
    return value.IntegerValue;
  }
}

export function createEffectiveDescriptorHandle(reference: HolonReference): EffectiveDescriptorHandle {
  return new EffectiveDescriptorHandle(reference, DESCRIPTOR_HANDLE_CONSTRUCTION);
}

/** Internal binding bridge; public consumers retain the read-only handle. */
export function unwrapEffectiveDescriptorHandle(handle: EffectiveDescriptorHandle): HolonReference {
  const reference = effectiveDescriptorReferences.get(handle);
  if (!reference) throw new TypeError('Invalid EffectiveDescriptorHandle');
  return reference;
}
