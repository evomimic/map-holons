use std::collections::HashSet;

use crate::descriptors::{
    accessor_helpers, inheritance::flatten_related_members, CommandDescriptor, Descriptor,
    TypeHeader,
};
use crate::reference_layer::HolonReference;
use core_types::HolonError;
use type_names::{CoreRelationshipTypeName, ToCommandName};

/// Runtime wrapper for the schema-backed `TransactionType` descriptor.
///
/// `TransactionType` is the command-scope descriptor model for transaction
/// affordances. It is not the live runtime `TransactionContext`.
pub struct TransactionDescriptor {
    holon: HolonReference,
}

impl TransactionDescriptor {
    /// Wraps an already-resolved descriptor holon reference.
    pub fn from_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    /// Projects the shared descriptor header view for this descriptor holon.
    pub fn header(&self) -> TypeHeader<'_> {
        TypeHeader::new(&self.holon)
    }

    /// Returns effective command descriptors across this descriptor's inheritance chain.
    pub fn afforded_commands(&self) -> Result<Vec<CommandDescriptor>, HolonError> {
        flatten_related_members(&self.holon, CoreRelationshipTypeName::AffordsCommand)
            .map(|members| members.into_iter().map(CommandDescriptor::from_holon).collect())
    }

    /// Finds an effective transaction command affordance by command descriptor type name.
    pub fn get_command_by_name(
        &self,
        command_name: impl ToCommandName,
    ) -> Result<CommandDescriptor, HolonError> {
        let requested_name = command_name.to_command_name();
        let requested = requested_name.to_string();
        let mut seen = HashSet::new();
        let mut found = None;

        // Detect duplicate effective declarations while scanning for the requested command.
        for descriptor in self.afforded_commands()? {
            let declaration_name = descriptor.command_name()?;
            let declaration_label = declaration_name.to_string();
            if !seen.insert(declaration_label.clone()) {
                return Err(HolonError::DuplicateInheritedDeclaration {
                    kind: "command".to_string(),
                    name: declaration_label,
                    descriptor: accessor_helpers::descriptor_label(&self.holon),
                });
            }
            if declaration_name == requested_name {
                found = Some(descriptor);
            }
        }

        found.ok_or_else(|| HolonError::DescriptorDeclarationNotFound {
            kind: "command".to_string(),
            name: requested,
            descriptor: accessor_helpers::descriptor_label(&self.holon),
        })
    }
}

impl From<HolonReference> for TransactionDescriptor {
    fn from(holon: HolonReference) -> Self {
        Self::from_holon(holon)
    }
}

impl Descriptor for TransactionDescriptor {
    fn holon(&self) -> &HolonReference {
        &self.holon
    }
}

#[cfg(test)]
const _: fn() = || {
    // Compile-time guard: this wrapper must continue implementing Descriptor.
    fn assert_impl<T: Descriptor>() {}
    assert_impl::<TransactionDescriptor>();
};
