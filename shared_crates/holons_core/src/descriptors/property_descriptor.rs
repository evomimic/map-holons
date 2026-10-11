use crate::descriptors::{accessor_helpers, Descriptor, TypeHeader, ValueDescriptor};
use crate::reference_layer::{HolonReference, ReadableHolon, WritableHolon};
use base_types::BaseValue;
use core_types::{HolonError, PropertyName};
use std::collections::HashSet;
use type_names::{CorePropertyTypeName, CoreRelationshipTypeName, ToPropertyName};

/// A property value resolved without writing, tagged by where it came from.
///
/// Returned by [`PropertyDescriptor::effective_value`]. The provenance lets a
/// caller validate an authored value strictly while accepting the descriptor's
/// own representation of its `DefaultValue` (the loader stores enum defaults as
/// variant-name tokens).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectiveValue {
    /// The holon's stored value; the default was never consulted.
    Authored(BaseValue),
    /// The descriptor-defined default, resolved because no value is stored. It
    /// is not written to the holon.
    Default(BaseValue),
}

/// Runtime wrapper for property descriptors.
///
/// This wrapper exposes property descriptor fields through the shared descriptor kernel.
pub struct PropertyDescriptor {
    holon: HolonReference,
}

impl PropertyDescriptor {
    /// Wraps an already-resolved descriptor holon reference.
    pub fn from_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    /// Projects the shared descriptor header view for this descriptor holon.
    pub fn header(&self) -> TypeHeader<'_> {
        TypeHeader::new(&self.holon)
    }

    /// Returns the runtime property name declared by this descriptor.
    pub fn property_name(&self) -> Result<PropertyName, HolonError> {
        Ok(PropertyName(self.header().type_name()?))
    }

    /// Resolves whether instances must provide this property, including inherited
    /// requiredness and the descriptor-defined Schema 2 default.
    pub fn is_required(&self) -> Result<bool, HolonError> {
        self.effective_is_value_required()
    }

    /// Returns the value descriptor reached through the required `ValueType` relationship.
    pub fn value_type(&self) -> Result<ValueDescriptor, HolonError> {
        let value_type = accessor_helpers::require_single_related(
            &self.holon,
            CoreRelationshipTypeName::ValueType,
        )?;
        Ok(ValueDescriptor::from_holon(value_type))
    }

    /// Resolves this property's effective value on `holon` without modifying it.
    ///
    /// Reads the stored value first. `Some` is returned as
    /// [`EffectiveValue::Authored`] without consulting any default; only an
    /// absent value resolves the descriptor's effective `DefaultValue`, returned
    /// as [`EffectiveValue::Default`]. A read error propagates immediately and
    /// never falls back to the default. `Ok(None)` means neither exists.
    ///
    /// This is the read-only counterpart of [`WritableHolon::populate_defaults`]:
    /// it never writes, so evaluating caller-owned arguments (e.g. a transient
    /// query definition) leaves them unchanged, and ordinary `property_value`
    /// reads keep their stored-value semantics. Validating the returned value
    /// against the property's value type is the caller's decision.
    pub fn effective_value<H>(&self, holon: &H) -> Result<Option<EffectiveValue>, HolonError>
    where
        H: ReadableHolon + ?Sized,
    {
        let property_name = self.property_name()?;
        if let Some(value) = holon.property_value(&property_name)? {
            return Ok(Some(EffectiveValue::Authored(value)));
        }
        Ok(self.effective_default_value()?.map(EffectiveValue::Default))
    }

    /// Populates this descriptor's effective default only when the target has
    /// no authored value and the effective property is required.
    pub(crate) fn populate_default_if_required_and_absent<H>(
        &self,
        target: &mut H,
    ) -> Result<(), HolonError>
    where
        H: ReadableHolon + WritableHolon + ?Sized,
    {
        let property_name = PropertyName(self.header().type_name()?);
        if target.property_value(&property_name)?.is_some() {
            return Ok(());
        }
        if !self.effective_is_value_required()? {
            return Ok(());
        }
        if let Some(default_value) = self.effective_default_value()? {
            target.with_property_value(property_name, default_value)?;
        }
        Ok(())
    }

    fn effective_is_value_required(&self) -> Result<bool, HolonError> {
        self.is_required_with_reader(&super::CurrentDescriptorReader)
    }

    /// Resolves requiredness and its describing-contract default in one prospective view.
    /// An omitted value is supplied by `D(P)`'s effective `IsValueRequired`
    /// member, never by a kernel Boolean default.
    pub fn is_required_with_reader<R: super::DescriptorReader>(
        &self,
        reader: &R,
    ) -> Result<bool, R::Error> {
        use super::{
            effective_property_value_with_reader, effective_relationship_targets_with_reader,
            resolve_describing_type_with_reader, DescribingTypeResolution,
        };
        for field in [CorePropertyTypeName::IsValueRequired, CorePropertyTypeName::IsRequired] {
            if let Some(value) = effective_property_value_with_reader(&self.holon, field, reader)? {
                return match value {
                    BaseValue::BooleanValue(value) => Ok(value.0),
                    other => {
                        Err(HolonError::UnexpectedValueType(format!("{other:?}"), "Boolean".into())
                            .into())
                    }
                };
            }
        }
        let descriptor = match resolve_describing_type_with_reader(&self.holon, reader)? {
            DescribingTypeResolution::Unique(descriptor) => descriptor,
            DescribingTypeResolution::Missing => {
                return Err(HolonError::MissingDescribedBy {
                    holon: self.holon.reference_id_string(),
                }
                .into())
            }
            DescribingTypeResolution::Multiple(targets) => {
                return Err(HolonError::MultipleDescribedBy {
                    holon: self.holon.reference_id_string(),
                    count: targets.len(),
                }
                .into())
            }
        };
        let mut seen = HashSet::new();
        let mut default_member = None;
        let requested_name = CorePropertyTypeName::IsValueRequired.to_property_name();
        for contribution in effective_relationship_targets_with_reader(
            &descriptor,
            CoreRelationshipTypeName::InstanceProperties,
            reader,
        )? {
            let declaration_name = PropertyName(accessor_helpers::require_string(
                &contribution.member,
                CorePropertyTypeName::TypeName,
            )?);
            let label = declaration_name.to_string();
            if !seen.insert(label.clone()) {
                return Err(HolonError::DuplicateInheritedDeclaration {
                    kind: "property".into(),
                    name: label,
                    descriptor: accessor_helpers::descriptor_label(&descriptor),
                }
                .into());
            }
            if declaration_name == requested_name {
                default_member = Some(contribution.member);
            }
        }
        let default_member =
            default_member.ok_or_else(|| HolonError::DescriptorDeclarationNotFound {
                kind: "property".into(),
                name: requested_name.to_string(),
                descriptor: accessor_helpers::descriptor_label(&descriptor),
            })?;
        match effective_property_value_with_reader(
            &default_member,
            CorePropertyTypeName::DefaultValue,
            reader,
        )? {
            Some(BaseValue::BooleanValue(value)) => Ok(value.0),
            Some(other) => {
                Err(HolonError::UnexpectedValueType(format!("{other:?}"), "Boolean".into()).into())
            }
            None => Err(HolonError::EmptyField("IsValueRequired.DefaultValue".into()).into()),
        }
    }

    fn effective_default_value(&self) -> Result<Option<BaseValue>, HolonError> {
        self.effective_property_value(CorePropertyTypeName::DefaultValue)
    }

    /// Resolves a descriptor-definition property self-first across `L(P)`.
    /// The property descriptor's own effective-member semantics are deliberately
    /// separate from the `L(D(H))` walk used to select this descriptor.
    fn effective_property_value(
        &self,
        name: CorePropertyTypeName,
    ) -> Result<Option<BaseValue>, HolonError> {
        accessor_helpers::effective_property_value(&self.holon, name)
    }
}

impl From<HolonReference> for PropertyDescriptor {
    fn from(holon: HolonReference) -> Self {
        Self::from_holon(holon)
    }
}

impl Descriptor for PropertyDescriptor {
    fn holon(&self) -> &HolonReference {
        &self.holon
    }
}

#[cfg(test)]
const _: fn() = || {
    // Compile-time guard: this wrapper must continue implementing Descriptor.
    fn assert_impl<T: Descriptor>() {}
    assert_impl::<PropertyDescriptor>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{
        build_context, new_declared_relationship_descriptor_holon, new_descriptor_holon,
        new_holon_type_descriptor, new_test_holon,
    };
    use crate::reference_layer::{ReadableHolon, WritableHolon};
    use base_types::MapString;
    use core_types::HolonError;
    use type_names::CoreRelationshipTypeName;

    #[test]
    fn wraps_reference_and_exposes_shared_header() -> Result<(), HolonError> {
        let context = build_context();
        let holon = HolonReference::from(&new_descriptor_holon(
            &context,
            "property-descriptor",
            "PropertyType",
            "Property",
        )?);

        let descriptor = PropertyDescriptor::from_holon(holon.clone());

        assert_eq!(descriptor.holon(), &holon);
        assert_eq!(descriptor.header().type_name()?, MapString("PropertyType".to_string()));

        Ok(())
    }

    #[test]
    fn populate_defaults_preserves_an_undescribed_holon() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon = new_test_holon(&context, "undescribed-instance")?;
        holon.with_property_value("Authored", "preserved")?;

        let before = holon.into_model()?;
        holon.populate_defaults()?;
        assert_eq!(holon.into_model()?, before);

        assert!(matches!(
            holon.property_value("Authored")?,
            Some(BaseValue::StringValue(value)) if value.0 == "preserved"
        ));
        Ok(())
    }

    #[test]
    fn populate_defaults_errors_for_multiple_described_by_targets() -> Result<(), HolonError> {
        let context = build_context();
        let descriptor_a = new_descriptor_holon(&context, "descriptor-a", "DescriptorA", "Holon")?;
        let descriptor_b = new_descriptor_holon(&context, "descriptor-b", "DescriptorB", "Holon")?;
        let mut holon = new_test_holon(&context, "multiple-descriptor-instance")?;
        holon.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![descriptor_a.into(), descriptor_b.into()],
        )?;

        assert!(matches!(
            holon.populate_defaults(),
            Err(HolonError::MultipleDescribedBy { count: 2, .. })
        ));

        Ok(())
    }

    #[test]
    fn attachment_preserves_progress_and_explicit_attempt_fills_nested_omissions(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let mut deferred = new_descriptor_holon(&context, "deferred", "Deferred", "Property")?;
        deferred.with_property_value(CorePropertyTypeName::DefaultValue, true)?;
        let mut required = new_descriptor_holon(&context, "required", "Required", "Property")?;
        required
            .with_property_value(CorePropertyTypeName::IsValueRequired, true)?
            .with_property_value(CorePropertyTypeName::DefaultValue, false)?;
        let mut optional = new_descriptor_holon(&context, "optional", "Optional", "Property")?;
        optional
            .with_property_value(CorePropertyTypeName::IsValueRequired, false)?
            .with_property_value(CorePropertyTypeName::DefaultValue, true)?;
        let mut descriptor = new_descriptor_holon(&context, "contract", "Contract", "Holon")?;
        descriptor.add_related_holons(
            CoreRelationshipTypeName::InstanceProperties,
            vec![deferred.clone().into(), required.into(), optional.into()],
        )?;
        let mut target = new_test_holon(&context, "target")?;
        target.with_descriptor(descriptor.into())?;

        assert_eq!(
            target.property_value("Required")?,
            Some(BaseValue::BooleanValue(base_types::MapBoolean(false)))
        );
        assert_eq!(target.property_value("Deferred")?, None);
        assert_eq!(target.property_value("Optional")?, None);

        let mut required_definition =
            new_descriptor_holon(&context, "required-definition", "IsValueRequired", "Property")?;
        required_definition
            .with_property_value(CorePropertyTypeName::IsValueRequired, false)?
            .with_property_value(CorePropertyTypeName::DefaultValue, true)?;
        let mut meta_property =
            new_descriptor_holon(&context, "meta-property", "MetaProperty", "Holon")?;
        meta_property.add_related_holons(
            CoreRelationshipTypeName::InstanceProperties,
            vec![required_definition.into()],
        )?;
        deferred.with_descriptor(meta_property.into())?;
        target.with_property_value("Required", true)?;

        target.populate_defaults()?;
        let before = target.into_model()?;
        target.populate_defaults()?;
        assert_eq!(target.into_model()?, before);
        assert_eq!(
            target.property_value("Required")?,
            Some(BaseValue::BooleanValue(base_types::MapBoolean(true)))
        );
        assert_eq!(
            target.property_value("Deferred")?,
            Some(BaseValue::BooleanValue(base_types::MapBoolean(true)))
        );
        assert_eq!(target.property_value("Optional")?, None);
        Ok(())
    }

    #[test]
    fn construction_completes_clones_and_staging_without_changing_source() -> Result<(), HolonError>
    {
        let context = build_context();
        let mut property = new_descriptor_holon(&context, "enabled", "Enabled", "Property")?;
        property
            .with_property_value(CorePropertyTypeName::IsValueRequired, true)?
            .with_property_value(CorePropertyTypeName::DefaultValue, false)?;
        let property = context.mutation().stage_new_holon(property)?;
        let mut descriptor = new_holon_type_descriptor(&context, "contract", "Contract")?;
        let described_by = new_declared_relationship_descriptor_holon(
            &context,
            "contract-described-by",
            "DescribedBy",
            descriptor.clone().into(),
            descriptor.clone().into(),
        )?;
        let described_by = context.mutation().stage_new_holon(described_by)?;
        descriptor.add_related_holons(
            CoreRelationshipTypeName::InstanceProperties,
            vec![property.into()],
        )?;
        descriptor.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![described_by.into()],
        )?;
        let descriptor = context.mutation().stage_new_holon(descriptor)?;
        let mut source = new_test_holon(&context, "source")?;
        source
            .add_related_holons(CoreRelationshipTypeName::DescribedBy, vec![descriptor.into()])?;
        assert_eq!(source.property_value("Enabled")?, None);
        source.with_property_value("Authored", "preserved")?;

        let transient_clone = context.clone_holon(&source.clone().into())?;
        let mut clone_source = context.mutation().stage_new_holon(source.clone())?;
        assert_eq!(
            clone_source.property_value("Enabled")?,
            Some(BaseValue::BooleanValue(base_types::MapBoolean(false)))
        );
        clone_source.remove_property_value("Enabled")?;
        let staged_clone = context
            .mutation()
            .stage_new_from_clone(clone_source.clone().into(), MapString("independent".into()))?;
        for completed in [HolonReference::from(transient_clone), staged_clone.into()] {
            assert_eq!(
                completed.property_value("Enabled")?,
                Some(BaseValue::BooleanValue(base_types::MapBoolean(false)))
            );
            assert_eq!(completed.property_value("Authored")?, source.property_value("Authored")?);
        }
        assert_eq!(source.property_value("Enabled")?, None);
        assert_eq!(clone_source.property_value("Enabled")?, None);
        Ok(())
    }

    /// A contract with an applicable default, an optional default, an absent default,
    /// and an authored value that must survive attachment.
    fn attachment_contract(
        context: &std::sync::Arc<crate::core_shared_objects::transactions::TransactionContext>,
    ) -> Result<HolonReference, HolonError> {
        let mut contract = new_holon_type_descriptor(context, "attach-contract", "AttachContract")?;
        for (name, required, default) in [
            ("Enabled", true, Some(false)),
            ("Optional", false, Some(true)),
            ("NoDefault", true, None),
            ("Authored", true, Some(false)),
        ] {
            let mut property = new_descriptor_holon(context, name, name, "Property")?;
            property.with_property_value(CorePropertyTypeName::IsValueRequired, required)?;
            if let Some(value) = default {
                property.with_property_value(CorePropertyTypeName::DefaultValue, value)?;
            }
            contract.add_related_holons(
                CoreRelationshipTypeName::InstanceProperties,
                vec![context.mutation().stage_new_holon(property)?.into()],
            )?;
        }
        Ok(context.mutation().stage_new_holon(contract)?.into())
    }

    #[test]
    fn transient_attachment_populates_only_required_absent_defaults() -> Result<(), HolonError> {
        let context = build_context();
        let mut subject = new_test_holon(&context, "transient-subject")?;
        subject.with_property_value("Authored", "preserved")?;
        subject.with_descriptor(attachment_contract(&context)?)?;
        assert_eq!(
            subject.property_value("Enabled")?,
            Some(BaseValue::BooleanValue(base_types::MapBoolean(false)))
        );
        assert_eq!(subject.property_value("Optional")?, None);
        assert_eq!(subject.property_value("NoDefault")?, None);
        assert_eq!(
            subject.property_value("Authored")?,
            Some(BaseValue::StringValue(MapString("preserved".into())))
        );
        Ok(())
    }

    #[test]
    fn staged_attachment_preserves_validation_state_and_removal_is_explicit(
    ) -> Result<(), HolonError> {
        use crate::core_shared_objects::holon::ValidationState;
        for state in [
            ValidationState::NoDescriptor,
            ValidationState::ValidationRequired,
            ValidationState::Validated,
            ValidationState::Invalid,
        ] {
            let context = build_context();
            let mut subject =
                context.mutation().stage_new_holon(new_test_holon(&context, "staged-subject")?)?;
            subject.replace_validation_outcome(state.clone(), Vec::new())?;
            let before = subject.validation_state()?;
            subject.with_descriptor(attachment_contract(&context)?)?;
            assert_eq!(subject.validation_state()?, before);
            assert_eq!(
                subject.property_value("Enabled")?,
                Some(BaseValue::BooleanValue(base_types::MapBoolean(false)))
            );
            subject.remove_property_value("Enabled")?;
            assert_eq!(subject.property_value("Enabled")?, None);
            subject.populate_defaults()?;
            assert_eq!(
                subject.property_value("Enabled")?,
                Some(BaseValue::BooleanValue(base_types::MapBoolean(false)))
            );
        }
        Ok(())
    }

    #[test]
    fn direct_descriptor_authoring_on_staged_holon_leaves_defaults_absent() -> Result<(), HolonError>
    {
        let context = build_context();
        let mut subject =
            context.mutation().stage_new_holon(new_test_holon(&context, "direct-subject")?)?;
        subject.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![attachment_contract(&context)?],
        )?;
        assert_eq!(subject.property_value("Enabled")?, None);
        subject.populate_defaults()?;
        assert_eq!(
            subject.property_value("Enabled")?,
            Some(BaseValue::BooleanValue(base_types::MapBoolean(false)))
        );
        Ok(())
    }

    #[test]
    fn attachment_error_retains_descriptor_and_ancestor_default() -> Result<(), HolonError> {
        let context = build_context();
        let mut healthy = new_descriptor_holon(&context, "healthy", "Healthy", "Property")?;
        healthy
            .with_property_value(CorePropertyTypeName::IsValueRequired, true)?
            .with_property_value(CorePropertyTypeName::DefaultValue, false)?;
        let mut ancestor = new_holon_type_descriptor(&context, "ancestor", "Ancestor")?;
        ancestor.add_related_holons(
            CoreRelationshipTypeName::InstanceProperties,
            vec![healthy.into()],
        )?;
        let empty_meta = new_holon_type_descriptor(&context, "empty-meta", "EmptyMeta")?;
        let mut failing = new_descriptor_holon(&context, "failing", "Failing", "Property")?;
        failing.with_descriptor(empty_meta.into())?;
        let mut contract =
            new_holon_type_descriptor(&context, "failing-contract", "FailingContract")?;
        contract.add_related_holons(CoreRelationshipTypeName::Extends, vec![ancestor.into()])?;
        contract.add_related_holons(
            CoreRelationshipTypeName::InstanceProperties,
            vec![failing.into()],
        )?;
        let contract: HolonReference = contract.into();
        let mut subject: HolonReference = new_test_holon(&context, "partial-error")?.into();
        assert!(
            matches!(subject.with_descriptor(contract.clone()), Err(HolonError::DescriptorDeclarationNotFound { name, .. }) if name == "IsValueRequired")
        );
        assert_eq!(subject.get_descriptor()?, Some(contract));
        assert_eq!(
            subject.property_value("Healthy")?,
            Some(BaseValue::BooleanValue(base_types::MapBoolean(false)))
        );
        assert_eq!(subject.property_value("Failing")?, None);
        Ok(())
    }

    #[test]
    fn inaccessible_attachment_returns_before_descriptor_reads() -> Result<(), HolonError> {
        let source_id = core_types::LocalId(vec![99]);
        let saved = crate::core_shared_objects::holon::SavedHolon::new(
            source_id.clone(),
            core_types::PropertyMap::new(),
            None,
            base_types::MapInteger(1),
        );
        let context = crate::descriptors::test_support::build_context_with_saved_holons(
            vec![saved],
            std::collections::HashMap::new(),
        );
        // The descriptor does not exist in the service. Any default read would fail
        // differently from the saved subject's write-access rejection.
        let mut subject = crate::SmartReference::new_from_id(
            context.space_read_handle(),
            core_types::HolonId::Local(core_types::LocalId(vec![99])),
        );
        let descriptor = HolonReference::smart_from_id(
            context.space_read_handle(),
            core_types::HolonId::Local(core_types::LocalId(vec![100])),
        );
        assert!(matches!(subject.with_descriptor(descriptor), Err(HolonError::NotAccessible(..))));
        Ok(())
    }

    #[test]
    fn defaults_added_before_and_after_update_staging_preserve_saved_source(
    ) -> Result<(), HolonError> {
        use crate::core_shared_objects::holon::{SavedHolon, StagedState};
        use crate::descriptors::test_support::build_context_with_saved_holons;
        use base_types::{MapBoolean, MapInteger};
        use core_types::{HolonId, LocalId, PropertyMap};
        use std::collections::HashMap;

        let id = |n| HolonId::Local(LocalId(vec![n]));
        let snapshots = [
            (1, "saved-source", "Source"),
            (2, "saved-contract", "Contract"),
            (3, "saved-property", "Enabled"),
            (4, "described-by", "DescribedBy"),
            (5, "declared-family", "DeclaredRelationshipType"),
        ]
        .into_iter()
        .map(|(n, key, name)| {
            let mut properties: PropertyMap = [
                (
                    CorePropertyTypeName::Key.to_property_name(),
                    BaseValue::StringValue(MapString(key.into())),
                ),
                (
                    CorePropertyTypeName::TypeName.to_property_name(),
                    BaseValue::StringValue(MapString(name.into())),
                ),
            ]
            .into();
            if n == 3 {
                properties.insert(
                    CorePropertyTypeName::IsValueRequired.to_property_name(),
                    BaseValue::BooleanValue(MapBoolean(true)),
                );
                properties.insert(
                    CorePropertyTypeName::DefaultValue.to_property_name(),
                    BaseValue::BooleanValue(MapBoolean(false)),
                );
            }
            SavedHolon::new(LocalId(vec![n]), properties, None, MapInteger(1))
        })
        .collect();
        let edges = HashMap::from([
            ((id(1), CoreRelationshipTypeName::DescribedBy.as_relationship_name()), vec![id(2)]),
            (
                (id(2), CoreRelationshipTypeName::InstanceProperties.as_relationship_name()),
                vec![id(3)],
            ),
            (
                (id(2), CoreRelationshipTypeName::InstanceRelationships.as_relationship_name()),
                vec![id(4)],
            ),
            ((id(4), CoreRelationshipTypeName::Extends.as_relationship_name()), vec![id(5)]),
        ]);
        let context = build_context_with_saved_holons(snapshots, edges);
        let source = crate::SmartReference::new_from_id(context.space_read_handle(), id(1));
        let before = source.into_model()?.property_map;
        let staged = context.mutation().stage_new_version(source.clone())?;
        assert_eq!(staged.staged_state()?, StagedState::ForUpdateNewVersion);
        assert_eq!(
            staged.property_value("Enabled")?,
            Some(BaseValue::BooleanValue(MapBoolean(false)))
        );
        assert_eq!(source.into_model()?.property_map, before);

        // Build a second unchanged update from an explicit source snapshot to isolate
        // the attachment-time write from clone-time population.
        let mut transient = context.mutation().new_holon(source.key()?)?;
        for (name, value) in &before {
            transient.with_property_value(name, value.clone())?;
        }
        let mut staged = crate::descriptors::test_support::stage_update_snapshot(
            &context,
            LocalId(vec![1]),
            transient,
        )?;
        assert_eq!(staged.staged_state()?, StagedState::ForUpdate);
        staged
            .with_descriptor(HolonReference::smart_from_id(context.space_read_handle(), id(2)))?;
        assert_eq!(staged.staged_state()?, StagedState::ForUpdateNewVersion);
        assert_eq!(
            staged.property_value("Enabled")?,
            Some(BaseValue::BooleanValue(MapBoolean(false)))
        );
        assert_eq!(source.into_model()?.property_map, before);
        Ok(())
    }

    /// A descriptor-backed holon with property `Mode` (default `Fast`) and an
    /// optional authored value.
    fn mode_fixture(
        authored: Option<&str>,
    ) -> Result<(PropertyDescriptor, crate::reference_layer::TransientReference), HolonError> {
        let context = build_context();
        let mut property = new_descriptor_holon(&context, "mode", "Mode", "Property")?;
        property
            .with_property_value(CorePropertyTypeName::IsValueRequired, true)?
            .with_property_value(CorePropertyTypeName::DefaultValue, "Fast")?;
        let mut holon = new_test_holon(&context, "moded")?;
        if let Some(value) = authored {
            holon.with_property_value("Mode", value)?;
        }
        Ok((PropertyDescriptor::from_holon(property.into()), holon))
    }

    #[test]
    fn effective_value_prefers_the_authored_value() -> Result<(), HolonError> {
        let (descriptor, holon) = mode_fixture(Some("Slow"))?;
        assert_eq!(
            descriptor.effective_value(&holon)?,
            Some(EffectiveValue::Authored(BaseValue::StringValue(MapString("Slow".into()))))
        );
        Ok(())
    }

    #[test]
    fn effective_value_resolves_an_absent_value_to_the_default_without_writing(
    ) -> Result<(), HolonError> {
        let (descriptor, holon) = mode_fixture(None)?;
        let before = holon.into_model()?;

        assert_eq!(
            descriptor.effective_value(&holon)?,
            Some(EffectiveValue::Default(BaseValue::StringValue(MapString("Fast".into()))))
        );
        assert_eq!(holon.property_value("Mode")?, None, "the default is not materialized");
        assert_eq!(holon.into_model()?, before, "the holon is unchanged");
        Ok(())
    }

    #[test]
    fn effective_value_without_a_value_or_default_is_none() -> Result<(), HolonError> {
        let context = build_context();
        let property = new_descriptor_holon(&context, "note", "Note", "Property")?;
        let holon = new_test_holon(&context, "noteless")?;
        assert_eq!(PropertyDescriptor::from_holon(property.into()).effective_value(&holon)?, None);
        Ok(())
    }

    #[test]
    fn effective_value_consults_the_default_only_when_the_value_is_absent() -> Result<(), HolonError>
    {
        // No local DefaultValue and two Extends targets: any default lookup
        // fails. An authored value must succeed without reaching it, even one
        // that is not valid for the property; only absence reaches the lookup.
        let context = build_context();
        let parent_a = new_descriptor_holon(&context, "parent-a", "ParentA", "Property")?;
        let parent_b = new_descriptor_holon(&context, "parent-b", "ParentB", "Property")?;
        let mut property = new_descriptor_holon(&context, "mode", "Mode", "Property")?;
        property.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![parent_a.into(), parent_b.into()],
        )?;
        let descriptor = PropertyDescriptor::from_holon(property.into());
        assert!(
            descriptor.effective_default_value().is_err(),
            "precondition: the default lookup itself fails"
        );

        let mut authored = new_test_holon(&context, "authored")?;
        authored.with_property_value("Mode", 42_i64)?;
        assert_eq!(
            descriptor.effective_value(&authored)?,
            Some(EffectiveValue::Authored(BaseValue::IntegerValue(base_types::MapInteger(42))))
        );

        let absent = new_test_holon(&context, "absent")?;
        assert!(descriptor.effective_value(&absent).is_err(), "absence reaches the lookup");
        Ok(())
    }

    const CONTROLLED_READ_FAILURE: &str = "controlled property read failure";

    /// A readable holon whose property reads fail with a controlled error and
    /// are counted; every other read delegates to a real transient holon.
    struct FailingPropertyRead {
        inner: crate::reference_layer::TransientReference,
        reads: std::cell::Cell<usize>,
    }

    impl FailingPropertyRead {
        fn new(inner: crate::reference_layer::TransientReference) -> Self {
            Self { inner, reads: std::cell::Cell::new(0) }
        }
    }

    impl crate::reference_layer::readable_impl::ReadableHolonImpl for FailingPropertyRead {
        fn all_related_holons_impl(&self) -> Result<crate::RelationshipMap, HolonError> {
            self.inner.all_related_holons_impl()
        }

        fn holon_id_impl(&self) -> Result<core_types::HolonId, HolonError> {
            self.inner.holon_id_impl()
        }

        fn predecessor_impl(&self) -> Result<Option<HolonReference>, HolonError> {
            self.inner.predecessor_impl()
        }

        fn property_value_impl(
            &self,
            _property_name: &PropertyName,
        ) -> Result<Option<core_types::PropertyValue>, HolonError> {
            self.reads.set(self.reads.get() + 1);
            Err(HolonError::FailedToAcquireLock(CONTROLLED_READ_FAILURE.to_string()))
        }

        fn key_impl(&self) -> Result<Option<MapString>, HolonError> {
            self.inner.key_impl()
        }

        fn related_holons_impl(
            &self,
            relationship_name: &core_types::RelationshipName,
        ) -> Result<
            std::sync::Arc<std::sync::RwLock<crate::core_shared_objects::HolonCollection>>,
            HolonError,
        > {
            self.inner.related_holons_impl(relationship_name)
        }

        fn versioned_key_impl(&self) -> Result<MapString, HolonError> {
            self.inner.versioned_key_impl()
        }

        fn property_map_impl(&self) -> Result<core_types::PropertyMap, HolonError> {
            self.inner.property_map_impl()
        }

        fn summarize_impl(&self) -> Result<String, HolonError> {
            self.inner.summarize_impl()
        }

        fn into_model_impl(&self) -> Result<core_types::HolonNodeModel, HolonError> {
            self.inner.into_model_impl()
        }

        fn is_accessible_impl(
            &self,
            access_type: crate::core_shared_objects::holon::state::AccessType,
        ) -> Result<(), HolonError> {
            self.inner.is_accessible_impl(access_type)
        }

        fn is_committed_source_impl(&self) -> Result<bool, HolonError> {
            self.inner.is_committed_source_impl()
        }

        fn holon_reference_impl(&self) -> HolonReference {
            self.inner.holon_reference_impl()
        }
    }

    fn assert_controlled_read_failure(result: Result<Option<EffectiveValue>, HolonError>) {
        match result {
            Err(HolonError::FailedToAcquireLock(message)) => {
                assert_eq!(message, CONTROLLED_READ_FAILURE)
            }
            other => panic!("expected the controlled read failure, got {other:?}"),
        }
    }

    #[test]
    fn effective_value_propagates_a_read_error_despite_an_available_default(
    ) -> Result<(), HolonError> {
        let (descriptor, holon) = mode_fixture(None)?;
        assert_eq!(
            descriptor.effective_value(&holon)?,
            Some(EffectiveValue::Default(BaseValue::StringValue(MapString("Fast".into())))),
            "precondition: the property has a usable default"
        );

        let failing = FailingPropertyRead::new(holon);
        assert_controlled_read_failure(descriptor.effective_value(&failing));
        assert_eq!(failing.reads.get(), 1, "the authored value is read once, not retried");
        Ok(())
    }

    #[test]
    fn effective_value_does_not_attempt_the_default_after_a_read_error() -> Result<(), HolonError> {
        // No local DefaultValue and two Extends targets: any default lookup
        // fails with its own error. The read error must surface instead.
        let context = build_context();
        let parent_a = new_descriptor_holon(&context, "parent-a", "ParentA", "Property")?;
        let parent_b = new_descriptor_holon(&context, "parent-b", "ParentB", "Property")?;
        let mut property = new_descriptor_holon(&context, "mode", "Mode", "Property")?;
        property.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![parent_a.into(), parent_b.into()],
        )?;
        let descriptor = PropertyDescriptor::from_holon(property.into());
        assert!(
            descriptor.effective_default_value().is_err(),
            "precondition: the default lookup itself fails"
        );

        let failing = FailingPropertyRead::new(new_test_holon(&context, "unreadable")?);
        assert_controlled_read_failure(descriptor.effective_value(&failing));
        assert_eq!(failing.reads.get(), 1);
        Ok(())
    }

    #[test]
    fn structural_accessors_return_declared_values() -> Result<(), HolonError> {
        let context = build_context();
        let value_type =
            new_descriptor_holon(&context, "string-value-type", "StringValueType", "Value")?;
        let mut holon = new_descriptor_holon(&context, "title-property", "Title", "Property")?;
        holon.with_property_value(CorePropertyTypeName::IsValueRequired, true)?;
        holon.add_related_holons(CoreRelationshipTypeName::ValueType, vec![value_type.into()])?;

        let descriptor = PropertyDescriptor::from_holon(holon.into());

        assert_eq!(descriptor.property_name()?.to_string(), "Title");
        assert!(descriptor.is_required()?);
        assert_eq!(
            descriptor.value_type()?.header().type_name()?,
            MapString("StringValueType".to_string())
        );

        Ok(())
    }

    #[test]
    fn property_name_errors_when_type_name_is_missing() -> Result<(), HolonError> {
        let context = build_context();
        let descriptor = PropertyDescriptor::from_holon(
            new_test_holon(&context, "property-without-type-name")?.into(),
        );

        assert!(matches!(
            descriptor.property_name(),
            Err(HolonError::EmptyField(field)) if field == "TypeName"
        ));

        Ok(())
    }

    #[test]
    fn value_type_errors_when_required_relationship_is_missing() -> Result<(), HolonError> {
        let context = build_context();
        let holon =
            new_descriptor_holon(&context, "missing-value-type", "MissingValueType", "Property")?;
        let descriptor = PropertyDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.value_type(),
            Err(HolonError::MissingRequiredRelationship { relationship, .. })
                if relationship == "ValueType"
        ));

        Ok(())
    }

    #[test]
    fn value_type_errors_when_multiple_targets_exist() -> Result<(), HolonError> {
        let context = build_context();
        let value_type_a =
            new_descriptor_holon(&context, "string-value-type-a", "StringValueTypeA", "Value")?;
        let value_type_b =
            new_descriptor_holon(&context, "string-value-type-b", "StringValueTypeB", "Value")?;
        let mut holon = new_descriptor_holon(
            &context,
            "multiple-value-types",
            "MultipleValueTypes",
            "Property",
        )?;
        holon.add_related_holons(
            CoreRelationshipTypeName::ValueType,
            vec![value_type_a.into(), value_type_b.into()],
        )?;

        let descriptor = PropertyDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.value_type(),
            Err(HolonError::MultipleRelatedHolons { relationship, count, .. })
                if relationship == "ValueType" && count == 2
        ));

        Ok(())
    }

    #[test]
    fn populate_default_populates_required_absent_value() -> Result<(), HolonError> {
        let context = build_context();
        let mut property =
            new_descriptor_holon(&context, "enabled-property", "Enabled", "Property")?;
        property
            .with_property_value(CorePropertyTypeName::IsValueRequired, true)?
            .with_property_value(CorePropertyTypeName::DefaultValue, false)?;
        let descriptor = PropertyDescriptor::from_holon(property.into());
        let mut target = new_test_holon(&context, "default-target")?;

        descriptor.populate_default_if_required_and_absent(&mut target)?;

        assert_eq!(
            target.property_value("Enabled")?,
            Some(base_types::BaseValue::BooleanValue(base_types::MapBoolean(false)))
        );
        Ok(())
    }

    #[test]
    fn populate_default_preserves_authored_value() -> Result<(), HolonError> {
        let context = build_context();
        let mut property =
            new_descriptor_holon(&context, "enabled-property", "Enabled", "Property")?;
        property
            .with_property_value(CorePropertyTypeName::IsValueRequired, true)?
            .with_property_value(CorePropertyTypeName::DefaultValue, false)?;
        let descriptor = PropertyDescriptor::from_holon(property.into());
        let mut target = new_test_holon(&context, "authored-target")?;
        target.with_property_value("Enabled", true)?;

        descriptor.populate_default_if_required_and_absent(&mut target)?;

        assert_eq!(
            target.property_value("Enabled")?,
            Some(base_types::BaseValue::BooleanValue(base_types::MapBoolean(true)))
        );
        Ok(())
    }

    #[test]
    fn populate_default_leaves_optional_value_absent() -> Result<(), HolonError> {
        let context = build_context();
        let mut property =
            new_descriptor_holon(&context, "optional-property", "Optional", "Property")?;
        property
            .with_property_value(CorePropertyTypeName::IsValueRequired, false)?
            .with_property_value(CorePropertyTypeName::DefaultValue, "ignored")?;
        let descriptor = PropertyDescriptor::from_holon(property.into());
        let mut target = new_test_holon(&context, "optional-target")?;

        descriptor.populate_default_if_required_and_absent(&mut target)?;

        assert_eq!(target.property_value("Optional")?, None);
        Ok(())
    }

    #[test]
    fn omitted_is_value_required_uses_the_meta_property_contract_default() -> Result<(), HolonError>
    {
        let context = build_context();

        let mut is_value_required_definition = new_descriptor_holon(
            &context,
            "is-value-required-definition",
            "IsValueRequired",
            "Property",
        )?;
        is_value_required_definition
            .with_property_value(CorePropertyTypeName::DefaultValue, false)?;

        let mut meta_property_type =
            new_descriptor_holon(&context, "meta-property-type", "MetaPropertyType", "Holon")?;
        meta_property_type.add_related_holons(
            CoreRelationshipTypeName::InstanceProperties,
            vec![is_value_required_definition.into()],
        )?;

        let mut optional_property =
            new_descriptor_holon(&context, "optional-property", "OptionalProperty", "Property")?;
        optional_property.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![meta_property_type.into()],
        )?;

        let descriptor = PropertyDescriptor::from_holon(optional_property.into());

        assert!(!descriptor.effective_is_value_required()?);
        Ok(())
    }

    #[test]
    fn omitted_requiredness_rejects_duplicate_contract_members() -> Result<(), HolonError> {
        let context = build_context();
        let mut first =
            new_descriptor_holon(&context, "first-requiredness", "IsValueRequired", "Property")?;
        first.with_property_value(CorePropertyTypeName::DefaultValue, false)?;
        let mut second =
            new_descriptor_holon(&context, "second-requiredness", "IsValueRequired", "Property")?;
        second.with_property_value(CorePropertyTypeName::DefaultValue, true)?;

        let mut meta_property_type =
            new_descriptor_holon(&context, "meta-property-type", "MetaPropertyType", "Holon")?;
        meta_property_type.add_related_holons(
            CoreRelationshipTypeName::InstanceProperties,
            vec![first.into(), second.into()],
        )?;
        let mut property = new_descriptor_holon(&context, "property", "Property", "Property")?;
        property.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![meta_property_type.into()],
        )?;
        let descriptor = PropertyDescriptor::from_holon(property.into());

        assert!(matches!(
            descriptor.is_required(),
            Err(HolonError::DuplicateInheritedDeclaration { kind, name, .. })
                if kind == "property" && name == "IsValueRequired"
        ));
        assert!(matches!(
            descriptor.is_required_with_reader(&super::super::CurrentDescriptorReader),
            Err(HolonError::DuplicateInheritedDeclaration { kind, name, .. })
                if kind == "property" && name == "IsValueRequired"
        ));
        Ok(())
    }

    #[test]
    fn authored_is_value_required_overrides_the_meta_property_contract_default(
    ) -> Result<(), HolonError> {
        let context = build_context();

        let mut is_value_required_definition = new_descriptor_holon(
            &context,
            "is-value-required-definition",
            "IsValueRequired",
            "Property",
        )?;
        is_value_required_definition
            .with_property_value(CorePropertyTypeName::DefaultValue, false)?;

        let mut meta_property_type =
            new_descriptor_holon(&context, "meta-property-type", "MetaPropertyType", "Holon")?;
        meta_property_type.add_related_holons(
            CoreRelationshipTypeName::InstanceProperties,
            vec![is_value_required_definition.into()],
        )?;

        let mut required_property =
            new_descriptor_holon(&context, "required-property", "RequiredProperty", "Property")?;
        required_property.with_property_value(CorePropertyTypeName::IsValueRequired, true)?;
        required_property.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![meta_property_type.into()],
        )?;

        let descriptor = PropertyDescriptor::from_holon(required_property.into());

        assert!(descriptor.effective_is_value_required()?);
        Ok(())
    }
}
