use crate::descriptors::{
    accessor_helpers, DanceResponseDescriptor, Descriptor, HolonDescriptor, TypeHeader,
};
use crate::dances::DanceImplementation;
use crate::reference_layer::{HolonReference, ReadableHolon};
use core_types::HolonError;
use type_names::CoreRelationshipTypeName;
use type_names::{DanceName, ToDanceName};

/// Runtime wrapper for dance descriptors.
pub struct DanceDescriptor {
    holon: HolonReference,
}

impl DanceDescriptor {
    pub fn from_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    pub fn header(&self) -> TypeHeader<'_> {
        TypeHeader::new(&self.holon)
    }

    pub fn dance_name(&self) -> Result<DanceName, HolonError> {
        Ok(self.header().type_name()?.to_dance_name())
    }

    pub fn request_type(&self) -> Result<Option<HolonDescriptor>, HolonError> {
        Ok(accessor_helpers::optional_single_related(
            &self.holon,
            CoreRelationshipTypeName::RequestType,
        )?
        .map(HolonDescriptor::from_holon))
    }

    pub fn response_type(&self) -> Result<DanceResponseDescriptor, HolonError> {
        let response = accessor_helpers::require_single_related(
            &self.holon,
            CoreRelationshipTypeName::Response,
        )?;
        Ok(DanceResponseDescriptor::from_holon(response))
    }

    pub fn implementation_candidates(&self) -> Result<Vec<DanceImplementation>, HolonError> {
        let implementations = self.holon.related_holons(CoreRelationshipTypeName::ForDance)?;
        let members = implementations
            .read()
            .map_err(|error| HolonError::FailedToAcquireLock(format!("{error}")))?
            .get_members()
            .clone();
        Ok(members.into_iter().map(DanceImplementation::from_holon).collect())
    }
}

impl From<HolonReference> for DanceDescriptor {
    fn from(holon: HolonReference) -> Self {
        Self::from_holon(holon)
    }
}

impl Descriptor for DanceDescriptor {
    fn holon(&self) -> &HolonReference {
        &self.holon
    }
}

#[cfg(test)]
const _: fn() = || {
    // Compile-time guard: this wrapper must continue implementing Descriptor.
    fn assert_impl<T: Descriptor>() {}
    assert_impl::<DanceDescriptor>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{build_context, new_descriptor_holon};
    use crate::reference_layer::WritableHolon;
    use base_types::MapString;
    use type_names::CoreRelationshipTypeName;

    #[test]
    fn wraps_reference_and_exposes_shared_header() -> Result<(), HolonError> {
        let context = build_context();
        let holon = HolonReference::from(&new_descriptor_holon(
            &context,
            "dance-descriptor",
            "Commit",
            "Holon",
        )?);

        let descriptor = DanceDescriptor::from_holon(holon.clone());

        assert_eq!(descriptor.holon(), &holon);
        assert_eq!(descriptor.header().type_name()?, MapString("Commit".to_string()));

        Ok(())
    }

    #[test]
    fn dance_name_uses_shared_type_name() -> Result<(), HolonError> {
        let context = build_context();
        let holon = new_descriptor_holon(&context, "query-dance", "Query", "Holon")?;
        let descriptor = DanceDescriptor::from_holon(holon.into());

        assert_eq!(descriptor.dance_name()?, type_names::DanceName(MapString("Query".to_string())));

        Ok(())
    }

    #[test]
    fn request_type_returns_none_when_missing() -> Result<(), HolonError> {
        let context = build_context();
        let holon = new_descriptor_holon(&context, "dance-no-request", "Query", "Holon")?;
        let descriptor = DanceDescriptor::from_holon(holon.into());

        assert!(descriptor.request_type()?.is_none());

        Ok(())
    }

    #[test]
    fn request_type_returns_single_related_target() -> Result<(), HolonError> {
        let context = build_context();
        let request_type = new_descriptor_holon(&context, "request-type", "Projection", "Holon")?;
        let mut holon = new_descriptor_holon(&context, "dance-with-request", "Query", "Holon")?;
        holon
            .add_related_holons(CoreRelationshipTypeName::RequestType, vec![request_type.into()])?;
        let descriptor = DanceDescriptor::from_holon(holon.into());

        assert_eq!(
            descriptor.request_type()?.expect("request type").header().type_name()?,
            MapString("Projection".to_string())
        );

        Ok(())
    }

    #[test]
    fn request_type_errors_when_multiple_targets_exist() -> Result<(), HolonError> {
        let context = build_context();
        let request_type_a =
            new_descriptor_holon(&context, "request-type-a", "ProjectionA", "Holon")?;
        let request_type_b =
            new_descriptor_holon(&context, "request-type-b", "ProjectionB", "Holon")?;
        let mut holon =
            new_descriptor_holon(&context, "dance-with-many-requests", "Query", "Holon")?;
        holon.add_related_holons(
            CoreRelationshipTypeName::RequestType,
            vec![request_type_a.into(), request_type_b.into()],
        )?;
        let descriptor = DanceDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.request_type(),
            Err(HolonError::MultipleRelatedHolons { relationship, count, .. })
                if relationship == "RequestType" && count == 2
        ));

        Ok(())
    }

    #[test]
    fn response_returns_single_related_target() -> Result<(), HolonError> {
        let context = build_context();
        let response_type =
            new_descriptor_holon(&context, "dance-response-type", "DanceResponseType", "Holon")?;
        let mut holon = new_descriptor_holon(&context, "dance-with-response", "Query", "Holon")?;
        holon.add_related_holons(CoreRelationshipTypeName::Response, vec![response_type.into()])?;
        let descriptor = DanceDescriptor::from_holon(holon.into());

        assert_eq!(
            descriptor.response_type()?.header().type_name()?,
            MapString("DanceResponseType".to_string())
        );

        Ok(())
    }

    #[test]
    fn response_errors_when_missing() -> Result<(), HolonError> {
        let context = build_context();
        let holon = new_descriptor_holon(&context, "dance-missing-response", "Query", "Holon")?;
        let descriptor = DanceDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.response_type(),
            Err(HolonError::MissingRequiredRelationship { relationship, .. })
                if relationship == "Response"
        ));

        Ok(())
    }

    #[test]
    fn response_body_returns_optional_related_target() -> Result<(), HolonError> {
        let context = build_context();
        let response_body = new_descriptor_holon(&context, "response-body", "Projection", "Holon")?;
        let mut response =
            new_descriptor_holon(&context, "dance-response", "DanceResponseType", "Holon")?;
        response.add_related_holons(
            CoreRelationshipTypeName::ResponseBody,
            vec![response_body.into()],
        )?;

        let descriptor = DanceResponseDescriptor::from_holon(response.into());

        assert_eq!(
            descriptor.response_body()?.expect("response body").header().type_name()?,
            MapString("Projection".to_string())
        );

        Ok(())
    }
}
