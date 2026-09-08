use super::*;
use core_types::{HolonId, LocalId};
use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
use holons_core::core_shared_objects::{Holon, ServiceRoutingPolicy};
use holons_core::reference_layer::HolonServiceApi;
use std::any::Any;

#[derive(Debug)]
struct NoStorage;

impl HolonServiceApi for NoStorage {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn commit_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &[StagedReference],
    ) -> Result<TransientReference, HolonError> {
        panic!("unexpected commit")
    }
    fn delete_holon_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &LocalId,
    ) -> Result<(), HolonError> {
        panic!("unexpected delete")
    }
    fn fetch_all_related_holons_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        panic!("unexpected storage read")
    }
    fn fetch_holon_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &HolonId,
    ) -> Result<Holon, HolonError> {
        panic!("unexpected storage read")
    }
    fn fetch_related_holons_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &HolonId,
        _: &RelationshipName,
    ) -> Result<HolonCollection, HolonError> {
        panic!("unexpected storage read")
    }
    fn get_all_holons_internal(
        &self,
        _: &Arc<TransactionContext>,
    ) -> Result<HolonCollection, HolonError> {
        panic!("unexpected storage read")
    }
    fn load_holons_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: TransientReference,
    ) -> Result<TransientReference, HolonError> {
        panic!("unexpected recursive load")
    }
}

fn context() -> Arc<TransactionContext> {
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        Arc::new(NoStorage),
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    space.get_transaction_manager().open_new_transaction(space.clone()).unwrap()
}

fn node(
    context: &Arc<TransactionContext>,
    key: &str,
    name: &str,
) -> Result<StagedReference, HolonError> {
    let mut transient = context.mutation().new_holon(Some(MapString(key.into())))?;
    transient.with_property_value("TypeName", name)?;
    context.mutation().stage_new_holon(transient)
}

fn relate(
    source: &mut StagedReference,
    relationship: &str,
    target: &StagedReference,
) -> Result<(), HolonError> {
    source.add_related_holons(relationship, vec![target.clone().into()])?;
    Ok(())
}

fn property(
    context: &Arc<TransactionContext>,
    name: &str,
    family: &StagedReference,
    value: &StagedReference,
) -> Result<StagedReference, HolonError> {
    let mut p = node(context, &format!("{name}.PropertyType"), name)?;
    p.with_property_value("IsValueRequired", false)?;
    relate(&mut p, "Extends", family)?;
    relate(&mut p, "ValueType", value)?;
    Ok(p)
}

#[test]
fn completion_materializes_imported_tokens_and_inherited_defaults_without_coercing_other_values(
) -> Result<(), HolonError> {
    let context = context();
    // Create the subject before its descriptors, as happens during bootstrap Pass 1.
    let mut subject = node(&context, "subject", "Subject")?;
    let enum_root = node(&context, "EnumValueType.ValueType", "RenamedEnumFamily")?;
    let property_root = node(&context, "PropertyType.TypeDescriptor", "Property")?;
    let mut enum_type = node(&context, "Example.ValueType", "Example")?;
    relate(&mut enum_type, "Extends", &enum_root)?;
    let string_type = node(&context, "Text.ValueType", "String")?;
    let mut owner = node(&context, "Owner.HolonType", "Owner")?;
    let mut base = property(&context, "BaseChoice", &property_root, &enum_type)?;
    base.with_property_value("IsValueRequired", true)?
        .with_property_value("DefaultValue", "Version")?;
    let mut inherited = property(&context, "InheritedChoice", &base, &enum_type)?;
    inherited.with_property_value("IsValueRequired", true)?;
    for name in ["AuthoredChoice", "NativeChoice", "WrongKind", "InvalidToken"] {
        let p = property(&context, name, &property_root, &enum_type)?;
        relate(&mut owner, "InstanceProperties", &p)?;
    }
    let mut text = property(&context, "Text", &property_root, &string_type)?;
    text.with_property_value("IsValueRequired", true)?
        .with_property_value("DefaultValue", "Version")?;
    relate(&mut owner, "InstanceProperties", &text)?;
    relate(&mut owner, "InstanceProperties", &inherited)?;
    subject
        .with_property_value("AuthoredChoice", "Lineage")?
        .with_property_value("NativeChoice", MapEnumValue(MapString("Version".into())))?
        .with_property_value("WrongKind", MapInteger(42))?
        .with_property_value("InvalidToken", " version ")?
        .with_property_value("Text", "Version")?;
    relate(&mut subject, "DescribedBy", &owner)?;
    // Attaching DescribedBy may already have populated the inherited string default.
    for _ in 0..2 {
        let (_, errors) = complete_loaded_values(&context)?;
        assert!(errors.is_empty(), "{errors:?}");
        for (name, token) in [
            ("InheritedChoice", "Version"),
            ("AuthoredChoice", "Lineage"),
            ("NativeChoice", "Version"),
            ("InvalidToken", " version "),
        ] {
            assert_eq!(
                subject.property_value(name)?,
                Some(BaseValue::EnumValue(MapEnumValue(MapString(token.into()))))
            );
        }
        assert_eq!(
            subject.property_value("Text")?,
            Some(BaseValue::StringValue(MapString("Version".into())))
        );
        assert_eq!(
            subject.property_value("WrongKind")?,
            Some(BaseValue::IntegerValue(MapInteger(42)))
        );
        assert_eq!(
            base.property_value("DefaultValue")?,
            Some(BaseValue::EnumValue(MapEnumValue(MapString("Version".into()))))
        );
        assert_eq!(inherited.property_value("DefaultValue")?, None);
        assert_eq!(
            text.property_value("DefaultValue")?,
            Some(BaseValue::StringValue(MapString("Version".into())))
        );
    }
    // A later creation receives the correctly typed default without loader conversion.
    let mut later = node(&context, "later", "Later")?;
    relate(&mut later, "DescribedBy", &owner)?;
    later.populate_defaults()?;
    assert_eq!(
        later.property_value("InheritedChoice")?,
        Some(BaseValue::EnumValue(MapEnumValue(MapString("Version".into()))))
    );
    Ok(())
}

#[test]
fn unresolved_value_type_is_a_completion_error_with_source_provenance() -> Result<(), HolonError> {
    let context = context();
    node(&context, "EnumValueType.ValueType", "Enum")?;
    let property_root = node(&context, "PropertyType.TypeDescriptor", "Property")?;
    let mut p = node(&context, "Broken.PropertyType", "Broken")?;
    relate(&mut p, "Extends", &property_root)?;
    p.with_property_value("DefaultValue", "Version")?;
    let (_, errors) = complete_loaded_values(&context)?;
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].source_loader_key, Some(MapString("Broken.PropertyType".into())));
    assert!(matches!(errors[0].error, HolonError::MissingRequiredRelationship { .. }));
    assert_eq!(
        p.property_value("DefaultValue")?,
        Some(BaseValue::StringValue(MapString("Version".into())))
    );
    Ok(())
}
