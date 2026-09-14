use super::*;
use crate::controller::tests::context;

fn node(context: &Arc<TransactionContext>, key: &str) -> StagedReference {
    let mut transient = context.mutation().new_holon(Some(key.into())).unwrap();
    transient.with_property_value(CorePropertyTypeName::TypeName, key).unwrap();
    transient.with_property_value(CorePropertyTypeName::IsAbstractType, false).unwrap();
    transient.with_property_value(CorePropertyTypeName::AllowsAdditionalProperties, false).unwrap();
    transient
        .with_property_value(CorePropertyTypeName::AllowsAdditionalRelationships, false)
        .unwrap();
    context.mutation().stage_new_holon(transient).unwrap()
}

fn edge(source: &mut StagedReference, name: &str, target: &StagedReference) {
    source.add_related_holons_ungoverned(name, vec![target.into()]).unwrap();
}

struct Fixture {
    context: Arc<TransactionContext>,
    book: StagedReference,
    person: StagedReference,
    book_type: StagedReference,
    declared: StagedReference,
}

impl Fixture {
    fn new() -> Self {
        let context = context();
        let mut book_type = node(&context, "BookType");
        let person_type = node(&context, "PersonType");
        let declared_root = node(&context, "DeclaredRelationshipType");
        let inverse_root = node(&context, "InverseRelationshipType");
        let mut declared = node(&context, "AuthoredBy");
        let mut inverse = node(&context, "AuthorOf");
        for (descriptor, root, source, target) in [
            (&mut declared, &declared_root, &book_type, &person_type),
            (&mut inverse, &inverse_root, &person_type, &book_type),
        ] {
            edge(descriptor, "Extends", root);
            edge(descriptor, "SourceType", source);
            edge(descriptor, "TargetType", target);
            descriptor.with_property_value(CorePropertyTypeName::IsDefinitional, false).unwrap();
            descriptor.with_property_value(CorePropertyTypeName::IsOrdered, false).unwrap();
            descriptor.with_property_value(CorePropertyTypeName::AllowsDuplicates, false).unwrap();
        }
        edge(&mut declared, "HasInverse", &inverse);
        edge(&mut book_type, "InstanceRelationships", &declared);
        let mut book = node(&context, "book");
        let mut person = node(&context, "person");
        edge(&mut book, "DescribedBy", &book_type);
        edge(&mut person, "DescribedBy", &person_type);
        Self { context, book, person, book_type, declared }
    }

    fn reference(
        &self,
        source: &StagedReference,
        name: &str,
        target: &StagedReference,
    ) -> TransientReference {
        let mut reference =
            self.context.mutation().new_holon(Some("relationship-reference".into())).unwrap();
        reference.with_property_value(CorePropertyTypeName::RelationshipName, name).unwrap();
        for (role, endpoint) in [
            (CoreRelationshipTypeName::ReferenceSource, source),
            (CoreRelationshipTypeName::ReferenceTarget, target),
        ] {
            let mut wrapper =
                self.context.mutation().new_holon(Some("endpoint-reference".into())).unwrap();
            wrapper
                .with_property_value(
                    CorePropertyTypeName::HolonKey,
                    endpoint.key().unwrap().unwrap(),
                )
                .unwrap();
            reference.add_related_holons(role, vec![wrapper.into()]).unwrap();
        }
        reference
    }
}

#[test]
fn declared_relationship_is_accepted_and_resolved_once() {
    let f = Fixture::new();
    let result = LoaderRefResolver::resolve_relationships(
        &f.context,
        vec![f.reference(&f.book, "AuthoredBy", &f.person)],
    )
    .unwrap();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.links_created, 1);
    assert_eq!(result.metrics.assembly.endpoint_resolution_calls, 1);
    assert_eq!(result.metrics.assessment.endpoint_resolution_calls, 0);
    assert_eq!(result.metrics.declaration_lookup_calls, 1);
}

#[test]
fn duplicate_inverse_references_keep_authored_targets_and_provenance() {
    let f = Fixture::new();
    let result = LoaderRefResolver::resolve_relationships(
        &f.context,
        vec![
            f.reference(&f.person, "AuthorOf", &f.book),
            f.reference(&f.person, "AuthorOf", &f.book),
        ],
    )
    .unwrap();
    assert_eq!(result.links_created, 1, "second write plan must be empty");
    assert_eq!(result.errors.len(), 2, "both authored references must be assessed");
    for error in result.errors {
        assert_eq!(error.source_loader_key, Some("person".into()));
        assert!(matches!(error.error, HolonError::InvalidRelationship(name, message)
            if name == "AuthorOf" && message.contains("AuthoredBy")
                && message.contains("opposite endpoint")));
    }
    assert_eq!(result.metrics.assembly.endpoint_resolution_calls, 2);
    assert_eq!(result.metrics.assessment.endpoint_resolution_calls, 0);
    assert_eq!(result.metrics.declaration_lookup_calls, 2);
}

#[test]
fn unknown_name_preserves_original_declaration_error() {
    let f = Fixture::new();
    let name = "Unknown".to_relationship_name();
    let expected =
        effective_relationship_declaration(&(&f.book).into(), name.clone()).err().unwrap();
    let mut metrics = ResolverMetrics::default();
    let actual = LoaderRefResolver::require_declared_relationship(
        &(&f.book).into(),
        &name,
        &[(&f.person).into()],
        &mut metrics,
    )
    .unwrap_err();
    assert_eq!(actual, expected);
    assert_eq!(metrics.declaration_lookup_calls, 1);
}

#[test]
fn late_declaration_is_available_before_any_assessment() {
    let mut f = Fixture::new();
    // Remove the declaration from the authored graph, then queue it AFTER its
    // first use. The instance type itself already exists: Extends ordering alone
    // cannot make this case pass if assembly and assessment are merged.
    f.book_type.remove_related_holons("InstanceRelationships", vec![(&f.declared).into()]).unwrap();
    let result = LoaderRefResolver::resolve_relationships(
        &f.context,
        vec![
            f.reference(&f.book, "AuthoredBy", &f.person),
            f.reference(&f.book_type, "InstanceRelationships", &f.declared),
        ],
    )
    .unwrap();
    // The deliberately minimal BookType has no metadescriptor, so assessment of
    // its own metadata edge fails. The earlier instance edge must NOT fail.
    assert_eq!(result.errors.len(), 1, "{:?}", result.errors);
    assert_eq!(result.errors[0].source_loader_key, Some("BookType".into()));
    assert!(matches!(result.errors[0].error, HolonError::MissingDescribedBy { .. }));
    assert_eq!(result.metrics.assembly.endpoint_resolution_calls, 2);
    assert_eq!(result.metrics.assessment.endpoint_resolution_calls, 0);
}

#[test]
fn endpoint_failure_is_counted_without_entering_assessment() {
    let f = Fixture::new();
    let mut broken = f.context.mutation().new_holon(Some("broken-reference".into())).unwrap();
    broken.with_property_value(CorePropertyTypeName::RelationshipName, "AuthoredBy").unwrap();
    let result = LoaderRefResolver::resolve_relationships(&f.context, vec![broken]).unwrap();
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.metrics.assembly.endpoint_resolution_calls, 1);
    assert_eq!(result.metrics.assessment.endpoint_resolution_calls, 0);
    assert_eq!(result.metrics.declaration_lookup_calls, 0);
}

#[test]
fn bootstrap_subpasses_count_their_own_endpoint_work() {
    let f = Fixture::new();
    let untyped = node(&f.context, "untyped");
    let base = node(&f.context, "BaseType");
    // Input order is immaterial: partitioning, not queue position, assigns work
    // to the two bootstrap phases. Neither reference enters assembly/assessment.
    let result = LoaderRefResolver::resolve_relationships(
        &f.context,
        vec![
            f.reference(&f.book_type, "Extends", &base),
            f.reference(&untyped, "DescribedBy", &f.book_type),
        ],
    )
    .unwrap();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.metrics.described_by.endpoint_resolution_calls, 1);
    assert_eq!(result.metrics.extends.endpoint_resolution_calls, 1);
    assert_eq!(result.metrics.assembly.endpoint_resolution_calls, 0);
    assert_eq!(result.metrics.assessment.endpoint_resolution_calls, 0);
    assert_eq!(result.metrics.declaration_lookup_calls, 0);
    assert_eq!(result.metrics.described_by.elapsed_micros, 0, "no HDK clock in unit tests");
}
