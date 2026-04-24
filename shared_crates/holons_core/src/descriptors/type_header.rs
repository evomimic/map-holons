use crate::reference_layer::{HolonReference, ReadableHolon};
use base_types::{BaseValue, MapString};
use core_types::HolonError;
use type_names::CorePropertyTypeName;

pub struct TypeHeader<'a> {
    holon: &'a HolonReference,
}

impl<'a> TypeHeader<'a> {
    #[allow(dead_code)]
    pub(crate) fn new(holon: &'a HolonReference) -> Self {
        Self { holon }
    }

    pub fn type_name(&self) -> Result<MapString, HolonError> {
        self.require_string(CorePropertyTypeName::TypeName)
    }

    pub fn type_name_plural(&self) -> Result<Option<MapString>, HolonError> {
        self.optional_string(CorePropertyTypeName::TypeNamePlural)
    }

    pub fn display_name(&self) -> Result<Option<MapString>, HolonError> {
        self.optional_string(CorePropertyTypeName::DisplayName)
    }

    pub fn display_name_plural(&self) -> Result<Option<MapString>, HolonError> {
        self.optional_string(CorePropertyTypeName::DisplayNamePlural)
    }

    pub fn description(&self) -> Result<Option<MapString>, HolonError> {
        self.optional_string(CorePropertyTypeName::Description)
    }

    pub fn is_abstract_type(&self) -> Result<bool, HolonError> {
        self.require_bool(CorePropertyTypeName::IsAbstractType)
    }

    pub fn instance_type_kind(&self) -> Result<MapString, HolonError> {
        self.require_string(CorePropertyTypeName::InstanceTypeKind)
    }

    fn require_string(&self, prop: CorePropertyTypeName) -> Result<MapString, HolonError> {
        let property_name = prop.as_property_name();
        match self.holon.property_value(prop)? {
            Some(BaseValue::StringValue(value)) => Ok(value),
            Some(other) => {
                Err(HolonError::UnexpectedValueType(format!("{:?}", other), "String".to_string()))
            }
            None => Err(HolonError::EmptyField(property_name.to_string())),
        }
    }

    fn optional_string(&self, prop: CorePropertyTypeName) -> Result<Option<MapString>, HolonError> {
        match self.holon.property_value(prop)? {
            Some(BaseValue::StringValue(value)) => Ok(Some(value)),
            Some(other) => {
                Err(HolonError::UnexpectedValueType(format!("{:?}", other), "String".to_string()))
            }
            None => Ok(None),
        }
    }

    fn require_bool(&self, prop: CorePropertyTypeName) -> Result<bool, HolonError> {
        let property_name = prop.as_property_name();
        match self.holon.property_value(prop)? {
            Some(BaseValue::BooleanValue(value)) => Ok(value.0),
            Some(other) => {
                Err(HolonError::UnexpectedValueType(format!("{:?}", other), "Boolean".to_string()))
            }
            None => Err(HolonError::EmptyField(property_name.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_shared_objects::space_manager::HolonSpaceManager;
    use crate::core_shared_objects::transactions::TransactionContext;
    use crate::core_shared_objects::{
        Holon, HolonCollection, RelationshipMap, ServiceRoutingPolicy,
    };
    use crate::reference_layer::{
        HolonServiceApi, StagedReference, TransientReference, WritableHolon,
    };
    use base_types::MapString;
    use core_types::{HolonId, LocalId, RelationshipName};
    use std::any::Any;
    use std::sync::Arc;

    #[derive(Debug)]
    struct TestHolonService;

    fn unreachable_in_type_header_tests<T>() -> Result<T, HolonError> {
        Err(HolonError::NotImplemented("TestHolonService".to_string()))
    }

    impl HolonServiceApi for TestHolonService {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn commit_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _staged_references: &[StagedReference],
        ) -> Result<TransientReference, HolonError> {
            unreachable_in_type_header_tests()
        }

        fn delete_holon_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _local_id: &LocalId,
        ) -> Result<(), HolonError> {
            unreachable_in_type_header_tests()
        }

        fn fetch_all_related_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _source_id: &HolonId,
        ) -> Result<RelationshipMap, HolonError> {
            unreachable_in_type_header_tests()
        }

        fn fetch_holon_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _id: &HolonId,
        ) -> Result<Holon, HolonError> {
            unreachable_in_type_header_tests()
        }

        fn fetch_related_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _source_id: &HolonId,
            _relationship_name: &RelationshipName,
        ) -> Result<HolonCollection, HolonError> {
            unreachable_in_type_header_tests()
        }

        fn get_all_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
        ) -> Result<HolonCollection, HolonError> {
            unreachable_in_type_header_tests()
        }

        fn load_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _bundle: TransientReference,
        ) -> Result<TransientReference, HolonError> {
            unreachable_in_type_header_tests()
        }
    }

    fn build_context() -> Arc<TransactionContext> {
        let holon_service: Arc<dyn HolonServiceApi> = Arc::new(TestHolonService);
        let space_manager = Arc::new(HolonSpaceManager::new_with_managers(
            None,
            holon_service,
            None,
            ServiceRoutingPolicy::BlockExternal,
        ));

        space_manager
            .get_transaction_manager()
            .open_new_transaction(Arc::clone(&space_manager))
            .expect("default transaction should open")
    }

    fn build_header_holon() -> Result<HolonReference, HolonError> {
        let mut holon = new_test_holon("header-holon")?;
        holon
            .with_property_value(CorePropertyTypeName::TypeName, "HolonType")
            .and_then(|holon| {
                holon.with_property_value(CorePropertyTypeName::TypeNamePlural, "HolonTypes")
            })
            .and_then(|holon| {
                holon.with_property_value(CorePropertyTypeName::DisplayName, "Holon Type")
            })
            .and_then(|holon| {
                holon.with_property_value(CorePropertyTypeName::DisplayNamePlural, "Holon Types")
            })
            .and_then(|holon| {
                holon.with_property_value(
                    CorePropertyTypeName::Description,
                    "Descriptor header test holon",
                )
            })
            .and_then(|holon| holon.with_property_value(CorePropertyTypeName::IsAbstractType, true))
            .and_then(|holon| {
                holon.with_property_value(CorePropertyTypeName::InstanceTypeKind, "Holon")
            })?;

        Ok(HolonReference::Transient(holon))
    }

    fn new_test_holon(key: &str) -> Result<TransientReference, HolonError> {
        build_context().mutation().new_holon(Some(MapString(key.to_string())))
    }

    #[test]
    fn header_accessors_return_expected_values() -> Result<(), HolonError> {
        let holon_ref = build_header_holon()?;
        let header = TypeHeader::new(&holon_ref);

        assert_eq!(header.type_name()?, MapString("HolonType".to_string()));
        assert_eq!(header.type_name_plural()?, Some(MapString("HolonTypes".to_string())));
        assert_eq!(header.display_name()?, Some(MapString("Holon Type".to_string())));
        assert_eq!(header.display_name_plural()?, Some(MapString("Holon Types".to_string())));
        assert_eq!(
            header.description()?,
            Some(MapString("Descriptor header test holon".to_string()))
        );
        assert!(header.is_abstract_type()?);
        assert_eq!(header.instance_type_kind()?, MapString("Holon".to_string()));

        Ok(())
    }

    #[test]
    fn type_name_errors_when_required_property_missing() -> Result<(), HolonError> {
        let mut holon = new_test_holon("missing-type-name")?;
        holon
            .with_property_value(CorePropertyTypeName::IsAbstractType, false)?
            .with_property_value(CorePropertyTypeName::InstanceTypeKind, "Holon")?;

        let holon_ref = HolonReference::Transient(holon);
        let header = TypeHeader::new(&holon_ref);

        assert!(matches!(
            header.type_name(),
            Err(HolonError::EmptyField(field)) if field == "TypeName"
        ));

        Ok(())
    }

    #[test]
    fn optional_accessors_return_none_when_absent() -> Result<(), HolonError> {
        let mut holon = new_test_holon("optional-header")?;
        holon
            .with_property_value(CorePropertyTypeName::TypeName, "HolonType")?
            .with_property_value(CorePropertyTypeName::IsAbstractType, false)?
            .with_property_value(CorePropertyTypeName::InstanceTypeKind, "Holon")?;

        let holon_ref = HolonReference::Transient(holon);
        let header = TypeHeader::new(&holon_ref);

        assert_eq!(header.type_name_plural()?, None);
        assert_eq!(header.display_name()?, None);
        assert_eq!(header.display_name_plural()?, None);
        assert_eq!(header.description()?, None);

        Ok(())
    }

    #[test]
    fn type_name_errors_when_property_value_has_wrong_type() -> Result<(), HolonError> {
        let mut holon = new_test_holon("wrong-type-name")?;
        holon
            .with_property_value(CorePropertyTypeName::TypeName, true)?
            .with_property_value(CorePropertyTypeName::IsAbstractType, false)?
            .with_property_value(CorePropertyTypeName::InstanceTypeKind, "Holon")?;

        let holon_ref = HolonReference::Transient(holon);
        let header = TypeHeader::new(&holon_ref);

        assert!(matches!(
            header.type_name(),
            Err(HolonError::UnexpectedValueType(_, expected)) if expected == "String"
        ));

        Ok(())
    }

    #[test]
    fn required_header_accessors_error_when_property_missing() -> Result<(), HolonError> {
        let mut missing_bool_holon = new_test_holon("missing-is-abstract")?;
        missing_bool_holon
            .with_property_value(CorePropertyTypeName::TypeName, "HolonType")?
            .with_property_value(CorePropertyTypeName::InstanceTypeKind, "Holon")?;

        let missing_bool_ref = HolonReference::Transient(missing_bool_holon);
        let missing_bool_header = TypeHeader::new(&missing_bool_ref);

        assert!(matches!(
            missing_bool_header.is_abstract_type(),
            Err(HolonError::EmptyField(field)) if field == "IsAbstractType"
        ));

        let mut missing_kind_holon = new_test_holon("missing-instance-type-kind")?;
        missing_kind_holon
            .with_property_value(CorePropertyTypeName::TypeName, "HolonType")?
            .with_property_value(CorePropertyTypeName::IsAbstractType, false)?;

        let missing_kind_ref = HolonReference::Transient(missing_kind_holon);
        let missing_kind_header = TypeHeader::new(&missing_kind_ref);

        assert!(matches!(
            missing_kind_header.instance_type_kind(),
            Err(HolonError::EmptyField(field)) if field == "InstanceTypeKind"
        ));

        Ok(())
    }
}
