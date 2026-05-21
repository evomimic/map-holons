use crate::descriptors::{Descriptor, TypeHeader};
use crate::reference_layer::HolonReference;
use base_types::MapString;
use core_types::HolonError;

/// Runtime wrapper for command descriptors.
///
/// Commands remain schema-declared `CommandType` holons. This wrapper exposes
/// descriptor-local command identity without introducing routing or execution
/// behavior.
pub struct CommandDescriptor {
    holon: HolonReference,
}

impl CommandDescriptor {
    /// Wraps an already-resolved descriptor holon reference.
    pub fn from_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    /// Projects the shared descriptor header view for this descriptor holon.
    pub fn header(&self) -> TypeHeader<'_> {
        TypeHeader::new(&self.holon)
    }

    /// Returns the command descriptor's canonical command name.
    pub fn command_name(&self) -> Result<MapString, HolonError> {
        self.header().type_name()
    }
}

impl From<HolonReference> for CommandDescriptor {
    fn from(holon: HolonReference) -> Self {
        Self::from_holon(holon)
    }
}

impl Descriptor for CommandDescriptor {
    fn holon(&self) -> &HolonReference {
        &self.holon
    }
}

#[cfg(test)]
const _: fn() = || {
    // Compile-time guard: this wrapper must continue implementing Descriptor.
    fn assert_impl<T: Descriptor>() {}
    assert_impl::<CommandDescriptor>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{build_context, new_descriptor_holon};

    #[test]
    fn wraps_reference_and_exposes_shared_header() -> Result<(), HolonError> {
        let context = build_context();
        let holon = HolonReference::from(&new_descriptor_holon(
            &context,
            "commit-command",
            "Commit",
            "Holon",
        )?);

        let descriptor = CommandDescriptor::from_holon(holon.clone());

        assert_eq!(descriptor.holon(), &holon);
        assert_eq!(descriptor.header().type_name()?, MapString("Commit".to_string()));

        Ok(())
    }

    #[test]
    fn command_name_uses_shared_type_name() -> Result<(), HolonError> {
        let context = build_context();
        let holon = new_descriptor_holon(
            &context,
            "begin-transaction-command",
            "BeginTransaction",
            "Holon",
        )?;

        let descriptor = CommandDescriptor::from_holon(holon.into());

        assert_eq!(descriptor.command_name()?, MapString("BeginTransaction".to_string()));

        Ok(())
    }

    #[test]
    fn from_holon_round_trips_through_from_trait() -> Result<(), HolonError> {
        let context = build_context();
        let holon = HolonReference::from(&new_descriptor_holon(
            &context,
            "query-command",
            "Query",
            "Holon",
        )?);

        let descriptor = CommandDescriptor::from(holon.clone());

        assert_eq!(descriptor.holon(), &holon);

        Ok(())
    }
}
