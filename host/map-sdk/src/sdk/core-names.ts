/**
 * MAP Core property names depended on by public SDK descriptor handles.
 *
 * This mirrors the deliberate Core-name dependencies in Rust without making
 * TypeScript infer descriptor semantics from arbitrary storage fields.
 */
export const CorePropertyName = {
  TypeName: 'TypeName',
  DisplayName: 'DisplayName',
  Description: 'Description',
  PropertyName: 'PropertyName',
  RelationshipName: 'RelationshipName',
} as const;

/** MAP Core descriptor relationships used by public descriptor handles. */
export const CoreRelationshipName = {
  ValueType: 'ValueType',
} as const;
