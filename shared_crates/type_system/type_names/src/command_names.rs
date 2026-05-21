use base_types::MapString;
use convert_case::{Case, Casing};
use std::fmt;
use strum_macros::VariantNames;

/// A strongly-typed wrapper around the shared descriptor `type_name` for command types.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommandName(pub MapString);

impl fmt::Display for CommandName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// Converts common command-name inputs into a typed `CommandName`.
pub trait ToCommandName {
    /// Returns the typed command name represented by this value.
    fn to_command_name(self) -> CommandName;
}

// --- Internal single point for canonicalization (ClassCase) ---
#[inline]
fn canonical_command_name<S: AsRef<str>>(command_name: S) -> CommandName {
    CommandName(MapString(command_name.as_ref().to_case(Case::UpperCamel)))
}

// --- to_command_name impls ---

impl ToCommandName for &str {
    fn to_command_name(self) -> CommandName {
        canonical_command_name(self)
    }
}

impl ToCommandName for String {
    fn to_command_name(self) -> CommandName {
        CommandName(MapString(self))
    }
}

impl ToCommandName for MapString {
    fn to_command_name(self) -> CommandName {
        CommandName(self)
    }
}

impl ToCommandName for &MapString {
    fn to_command_name(self) -> CommandName {
        CommandName(self.clone())
    }
}

impl ToCommandName for CoreCommandTypeName {
    fn to_command_name(self) -> CommandName {
        self.as_command_name()
    }
}

impl ToCommandName for &CoreCommandTypeName {
    fn to_command_name(self) -> CommandName {
        self.clone().as_command_name()
    }
}

impl ToCommandName for CommandName {
    fn to_command_name(self) -> CommandName {
        self
    }
}

impl ToCommandName for &CommandName {
    fn to_command_name(self) -> CommandName {
        self.clone()
    }
}

/// Stable MAP Core command type names backed by concrete `CommandType` descriptors.
#[derive(Debug, Clone, PartialEq, Eq, VariantNames)]
pub enum CoreCommandTypeName {
    BeginTransaction,
    CloneHolon,
    GetEssentialContent,
    Summarize,
    GetHolonId,
    GetPredecessor,
    GetKey,
    GetVersionedKey,
    GetPropertyValue,
    GetRelatedHolons,
    WithPropertyValue,
    RemovePropertyValue,
    AddRelatedHolons,
    RemoveRelatedHolons,
    WithDescriptor,
    Commit,
    UndoLast,
    RedoLast,
    UndoToMarker,
    RedoToMarker,
    LoadHolons,
    Dance,
    Query,
    GetAllHolons,
    GetStagedHolonByBaseKey,
    GetStagedHolonsByBaseKey,
    GetStagedHolonByVersionedKey,
    GetTransientHolonByBaseKey,
    GetTransientHolonByVersionedKey,
    GetStagedCount,
    GetTransientCount,
    NewHolon,
    StageNewHolon,
    StageNewFromClone,
    StageNewVersion,
    StageNewVersionFromId,
    DeleteHolon,
}

impl CoreCommandTypeName {
    /// Canonical command type name in ClassCase (UpperCamel).
    pub fn as_command_name(&self) -> CommandName {
        // Use the Rust variant identifier as the core inventory source, then apply
        // the shared type-name case convention used by the neighboring modules.
        let command_name = format!("{self:?}").to_case(Case::UpperCamel);
        CommandName(MapString(command_name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strum::VariantNames as _;

    fn expected_command_name(command_name: &str) -> CommandName {
        CommandName(MapString(command_name.to_string()))
    }

    fn all_core_command_type_names() -> Vec<CoreCommandTypeName> {
        vec![
            CoreCommandTypeName::BeginTransaction,
            CoreCommandTypeName::CloneHolon,
            CoreCommandTypeName::GetEssentialContent,
            CoreCommandTypeName::Summarize,
            CoreCommandTypeName::GetHolonId,
            CoreCommandTypeName::GetPredecessor,
            CoreCommandTypeName::GetKey,
            CoreCommandTypeName::GetVersionedKey,
            CoreCommandTypeName::GetPropertyValue,
            CoreCommandTypeName::GetRelatedHolons,
            CoreCommandTypeName::WithPropertyValue,
            CoreCommandTypeName::RemovePropertyValue,
            CoreCommandTypeName::AddRelatedHolons,
            CoreCommandTypeName::RemoveRelatedHolons,
            CoreCommandTypeName::WithDescriptor,
            CoreCommandTypeName::Commit,
            CoreCommandTypeName::UndoLast,
            CoreCommandTypeName::RedoLast,
            CoreCommandTypeName::UndoToMarker,
            CoreCommandTypeName::RedoToMarker,
            CoreCommandTypeName::LoadHolons,
            CoreCommandTypeName::Dance,
            CoreCommandTypeName::Query,
            CoreCommandTypeName::GetAllHolons,
            CoreCommandTypeName::GetStagedHolonByBaseKey,
            CoreCommandTypeName::GetStagedHolonsByBaseKey,
            CoreCommandTypeName::GetStagedHolonByVersionedKey,
            CoreCommandTypeName::GetTransientHolonByBaseKey,
            CoreCommandTypeName::GetTransientHolonByVersionedKey,
            CoreCommandTypeName::GetStagedCount,
            CoreCommandTypeName::GetTransientCount,
            CoreCommandTypeName::NewHolon,
            CoreCommandTypeName::StageNewHolon,
            CoreCommandTypeName::StageNewFromClone,
            CoreCommandTypeName::StageNewVersion,
            CoreCommandTypeName::StageNewVersionFromId,
            CoreCommandTypeName::DeleteHolon,
        ]
    }

    #[test]
    fn test_variant_string_conversion() {
        let variants = all_core_command_type_names();
        assert_eq!(CoreCommandTypeName::VARIANTS.len(), variants.len());

        for (variant_name, core_command_type_name) in
            CoreCommandTypeName::VARIANTS.iter().zip(variants)
        {
            assert_eq!(
                expected_command_name(variant_name),
                core_command_type_name.as_command_name()
            );
        }
    }

    #[test]
    fn test_to_command_name_accepts_common_input_shapes() {
        assert_eq!(expected_command_name("GetKey"), "get_key".to_command_name());
        assert_eq!(
            expected_command_name("AlreadyCanonical"),
            String::from("AlreadyCanonical").to_command_name()
        );

        let map_string = MapString("GetPropertyValue".to_string());
        assert_eq!(expected_command_name("GetPropertyValue"), map_string.clone().to_command_name());
        assert_eq!(expected_command_name("GetPropertyValue"), (&map_string).to_command_name());

        let command_name = expected_command_name("GetRelatedHolons");
        assert_eq!(
            expected_command_name("GetRelatedHolons"),
            command_name.clone().to_command_name()
        );
        assert_eq!(expected_command_name("GetRelatedHolons"), (&command_name).to_command_name());

        let core_command_type_name = CoreCommandTypeName::GetEssentialContent;
        assert_eq!(
            expected_command_name("GetEssentialContent"),
            core_command_type_name.clone().to_command_name()
        );
        assert_eq!(
            expected_command_name("GetEssentialContent"),
            (&core_command_type_name).to_command_name()
        );
    }
}
