
// Auto-generated enum from enum_template.rs
use std::str::FromStr;
use std::fmt;
use strum_macros::EnumIter;

#[derive(Debug, Clone, EnumIter, Default, PartialEq, Eq)]
pub enum CorePropertyTypeNameName {
    #[default]
    AllowDuplicates,
    TypeKind,
    DeletionSemantic,
    DescriptorName,
    Description,
    IsBuiltinType,
    IsDependent,
    IsOrdered,
    IsValueType,
    Label,
    MaxCardinality,
    MaxLength,
    MaxValue,
    MinCardinality,
    MinLength,
    MinValue,
    Name,
    PropertyTypeName,
    RelationshipName,
    SchemaName,
    TypeName,
    VariantName,
    VariantOrder,
    Version,
}

impl fmt::Display for CorePropertyTypeNameName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CorePropertyTypeNameName::AllowDuplicates => write!(f, "AllowDuplicates"),
            CorePropertyTypeNameName::TypeKind => write!(f, "TypeKind"),
            CorePropertyTypeNameName::DeletionSemantic => write!(f, "DeletionSemantic"),
            CorePropertyTypeNameName::DescriptorName => write!(f, "DescriptorName"),
            CorePropertyTypeNameName::Description => write!(f, "Description"),
            CorePropertyTypeNameName::IsBuiltinType => write!(f, "IsBuiltinType"),
            CorePropertyTypeNameName::IsDependent => write!(f, "IsDependent"),
            CorePropertyTypeNameName::IsOrdered => write!(f, "IsOrdered"),
            CorePropertyTypeNameName::IsValueType => write!(f, "IsValueType"),
            CorePropertyTypeNameName::Label => write!(f, "Label"),
            CorePropertyTypeNameName::MaxCardinality => write!(f, "MaxCardinality"),
            CorePropertyTypeNameName::MaxLength => write!(f, "MaxLength"),
            CorePropertyTypeNameName::MaxValue => write!(f, "MaxValue"),
            CorePropertyTypeNameName::MinCardinality => write!(f, "MinCardinality"),
            CorePropertyTypeNameName::MinLength => write!(f, "MinLength"),
            CorePropertyTypeNameName::MinValue => write!(f, "MinValue"),
            CorePropertyTypeNameName::Name => write!(f, "Name"),
            CorePropertyTypeNameName::PropertyTypeName => write!(f, "PropertyTypeName"),
            CorePropertyTypeNameName::RelationshipName => write!(f, "RelationshipName"),
            CorePropertyTypeNameName::SchemaName => write!(f, "SchemaName"),
            CorePropertyTypeNameName::TypeName => write!(f, "TypeName"),
            CorePropertyTypeNameName::VariantName => write!(f, "VariantName"),
            CorePropertyTypeNameName::VariantOrder => write!(f, "VariantOrder"),
            CorePropertyTypeNameName::Version => write!(f, "Version"),
        }
    }
}

impl FromStr for CorePropertyTypeNameName {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "AllowDuplicates" => Ok(CorePropertyTypeNameName::AllowDuplicates),
            "TypeKind" => Ok(CorePropertyTypeNameName::TypeKind),
            "DeletionSemantic" => Ok(CorePropertyTypeNameName::DeletionSemantic),
            "DescriptorName" => Ok(CorePropertyTypeNameName::DescriptorName),
            "Description" => Ok(CorePropertyTypeNameName::Description),
            "IsBuiltinType" => Ok(CorePropertyTypeNameName::IsBuiltinType),
            "IsDependent" => Ok(CorePropertyTypeNameName::IsDependent),
            "IsOrdered" => Ok(CorePropertyTypeNameName::IsOrdered),
            "IsValueType" => Ok(CorePropertyTypeNameName::IsValueType),
            "Label" => Ok(CorePropertyTypeNameName::Label),
            "MaxCardinality" => Ok(CorePropertyTypeNameName::MaxCardinality),
            "MaxLength" => Ok(CorePropertyTypeNameName::MaxLength),
            "MaxValue" => Ok(CorePropertyTypeNameName::MaxValue),
            "MinCardinality" => Ok(CorePropertyTypeNameName::MinCardinality),
            "MinLength" => Ok(CorePropertyTypeNameName::MinLength),
            "MinValue" => Ok(CorePropertyTypeNameName::MinValue),
            "Name" => Ok(CorePropertyTypeNameName::Name),
            "PropertyTypeName" => Ok(CorePropertyTypeNameName::PropertyTypeName),
            "RelationshipName" => Ok(CorePropertyTypeNameName::RelationshipName),
            "SchemaName" => Ok(CorePropertyTypeNameName::SchemaName),
            "TypeName" => Ok(CorePropertyTypeNameName::TypeName),
            "VariantName" => Ok(CorePropertyTypeNameName::VariantName),
            "VariantOrder" => Ok(CorePropertyTypeNameName::VariantOrder),
            "Version" => Ok(CorePropertyTypeNameName::Version),
            _ => Err(()),
        }
    }
}
