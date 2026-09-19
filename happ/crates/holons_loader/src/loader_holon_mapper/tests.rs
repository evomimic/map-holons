use super::*;
use crate::controller::tests::context;

#[test]
fn pass_one_stages_undescribed_input_and_preserves_raw_relationship_references() {
    let context = context();
    let mut loader = context.mutation().new_holon(Some("ValueType.TypeDescriptor".into())).unwrap();
    loader.with_property_value(CorePropertyTypeName::TypeName, "ValueType").unwrap();
    loader.with_property_value(StartUtf8ByteOffset, MapInteger(42)).unwrap();
    let mut relationship = context.mutation().new_holon(Some("relationship".into())).unwrap();
    relationship
        .with_property_value(CorePropertyTypeName::RelationshipName, "DescribedBy")
        .unwrap();
    let mut endpoint = context.mutation().new_holon(Some("endpoint".into())).unwrap();
    endpoint
        .with_property_value(CorePropertyTypeName::HolonKey, "MetaValueType.MetaTypeDescriptor")
        .unwrap();
    relationship
        .add_related_holons(
            CoreRelationshipTypeName::ReferenceTarget,
            vec![endpoint.clone().into()],
        )
        .unwrap();
    loader.add_related_holons(HasRelationshipReference, vec![relationship.clone().into()]).unwrap();
    let mut bundle = context.mutation().new_holon(Some("bundle".into())).unwrap();
    bundle.add_related_holons(BundleMembers, vec![loader.clone().into()]).unwrap();

    let output = LoaderHolonMapper::map_bundle(&context, bundle).unwrap();
    assert!(output.errors.is_empty(), "{:?}", output.errors);
    assert_eq!(output.staged_count, 1);
    let staged = context.staged_references().unwrap().pop().unwrap();
    assert_eq!(
        staged.property_value(CorePropertyTypeName::TypeName).unwrap(),
        loader.property_value(CorePropertyTypeName::TypeName).unwrap()
    );
    assert_eq!(staged.property_value(StartUtf8ByteOffset).unwrap(), None);
    assert_eq!(staged.all_related_holons().unwrap().count(), 0);
    assert_eq!(
        loader.property_value(StartUtf8ByteOffset).unwrap(),
        Some(BaseValue::IntegerValue(MapInteger(42)))
    );
    assert_eq!(output.queued_relationship_references.len(), 1);
    let queued = &output.queued_relationship_references[0];
    assert_ne!(queued.temporary_id(), relationship.temporary_id());
    assert_eq!(
        queued
            .related_holons(CoreRelationshipTypeName::ReferenceTarget)
            .unwrap()
            .read()
            .unwrap()
            .get_members(),
        &vec![endpoint.into()]
    );
}
