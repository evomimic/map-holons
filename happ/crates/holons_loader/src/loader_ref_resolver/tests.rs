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
}

#[test]
fn assembly_preserves_unlicensed_names_without_inverting_them() {
    // Assembly preserves authored input for validation against the completed
    // graph; it must not infer or synthesize a forward edge from an inverse name.
    for name in ["AuthorOf", "Unknown"] {
        let f = Fixture::new();
        let result = LoaderRefResolver::resolve_relationships(
            &f.context,
            vec![f.reference(&f.person, name, &f.book), f.reference(&f.person, name, &f.book)],
        )
        .unwrap();
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.links_created, 1, "replayed authored edges are deduplicated");
        assert_eq!(
            f.person.related_holons(name).unwrap().read().unwrap().get_members(),
            &vec![HolonReference::from(&f.book)]
        );
        assert!(f
            .book
            .related_holons("AuthoredBy")
            .unwrap()
            .read()
            .unwrap()
            .get_members()
            .is_empty());
        assert_eq!(result.metrics.assembly.endpoint_resolution_calls, 2);
    }
}

#[test]
fn assembly_allows_declarations_before_or_after_their_use() {
    for declaration_first in [false, true] {
        let mut f = Fixture::new();
        f.book_type
            .remove_related_holons("InstanceRelationships", vec![(&f.declared).into()])
            .unwrap();
        let mut references = vec![
            f.reference(&f.book, "AuthoredBy", &f.person),
            f.reference(&f.book_type, "InstanceRelationships", &f.declared),
        ];
        if declaration_first {
            references.reverse();
        }
        let result = LoaderRefResolver::resolve_relationships(&f.context, references).unwrap();
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.links_created, 2);
        assert_eq!(
            f.book.related_holons("AuthoredBy").unwrap().read().unwrap().get_members(),
            &vec![HolonReference::from(&f.person)]
        );
        effective_relationship_declaration(&(&f.book).into(), "AuthoredBy").unwrap();
        assert_eq!(result.metrics.assembly.endpoint_resolution_calls, 2);
    }
}

#[test]
fn endpoint_failure_is_counted_without_declaredness_lookup() {
    let f = Fixture::new();
    let mut broken = f.context.mutation().new_holon(Some("broken-reference".into())).unwrap();
    broken.with_property_value(CorePropertyTypeName::RelationshipName, "AuthoredBy").unwrap();
    let result = LoaderRefResolver::resolve_relationships(&f.context, vec![broken]).unwrap();
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.metrics.assembly.endpoint_resolution_calls, 1);
}

#[test]
fn bootstrap_subpasses_count_their_own_endpoint_work() {
    let f = Fixture::new();
    let untyped = node(&f.context, "untyped");
    let base = node(&f.context, "BaseType");
    // Input order is immaterial: partitioning, not queue position, assigns work
    // to the two bootstrap phases. Neither reference enters ordinary assembly.
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
    assert_eq!(result.metrics.described_by.elapsed_micros, 0, "no HDK clock in unit tests");
}
