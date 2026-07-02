use crate::descriptors::{RelationshipDescriptor, RelationshipDirection};

/// Query/navigation direction for traversing a relationship from an endpoint.
///
/// This is separate from [`RelationshipDirection`], which classifies whether a
/// matched relationship descriptor is the declared or inverse descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraversalDirection {
    /// Traverse toward holons related to this endpoint by incoming relationship edges.
    Inbound,
    /// Traverse toward holons related to this endpoint by outgoing relationship edges.
    Outbound,
}

/// A relationship descriptor matched for a requested traversal form.
pub struct QualifiedRelationship {
    /// The matched relationship descriptor as requested by the caller.
    pub descriptor: RelationshipDescriptor,
    /// Whether `descriptor` is the declared or inverse relationship descriptor.
    pub descriptor_direction: RelationshipDirection,
    /// The caller's requested query/navigation direction.
    pub traversal_direction: TraversalDirection,
}
