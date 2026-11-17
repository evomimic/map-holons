use super::EnumValueDescriptor;
use crate::descriptors::{inheritance::effective_relationship_targets, Descriptor};
use crate::{DefinitionIdentity, ReadableHolon};
use base_types::MapString;
use core_types::HolonError;
use std::collections::BTreeMap;
use type_names::CoreRelationshipTypeName;

/// Exact local evidence for one authored enum member.
#[derive(Clone, Debug)]
pub struct EnumMember {
    pub identity: DefinitionIdentity,
    pub token: MapString,
    pub declared_on: DefinitionIdentity,
    pub is_abstract: bool,
}

impl EnumMember {
    /// Acquires exact contribution evidence. Outer errors are acquisition failures;
    /// inner errors describe missing or mistyped local declaration fields.
    pub fn from_contribution(
        contribution: &crate::EffectiveRelationshipMember,
        context: Option<
            &std::sync::Arc<crate::core_shared_objects::transactions::TransactionContext>,
        >,
    ) -> Result<Result<Self, HolonError>, HolonError> {
        use base_types::BaseValue;
        use type_names::CorePropertyTypeName;
        let identity = DefinitionIdentity::for_optional_context(&contribution.member, context)?;
        let declared_on =
            DefinitionIdentity::for_optional_context(&contribution.declared_on, context)?;
        let token = contribution.member.property_value(CorePropertyTypeName::TypeName)?;
        let abstractness =
            contribution.member.property_value(CorePropertyTypeName::IsAbstractType)?;
        Ok((|| {
            let token = match token {
                Some(BaseValue::StringValue(value)) => value,
                None => return Err(HolonError::EmptyField("TypeName".into())),
                Some(other) => {
                    return Err(HolonError::UnexpectedValueType(
                        format!("{other:?}"),
                        "String".into(),
                    ))
                }
            };
            let is_abstract = match abstractness {
                Some(BaseValue::BooleanValue(value)) => value.0,
                None => return Err(HolonError::EmptyField("IsAbstractType".into())),
                Some(other) => {
                    return Err(HolonError::UnexpectedValueType(
                        format!("{other:?}"),
                        "Boolean".into(),
                    ))
                }
            };
            Ok(Self { identity, token, declared_on, is_abstract })
        })())
    }
}

/// Membership counts distinct concrete definitions, never abstract anchors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnumMembership {
    Member(DefinitionIdentity),
    Undeclared,
    Ambiguous(Vec<DefinitionIdentity>),
}

/// Owned enum evidence in authored order, suitable for assessment-local preparation.
#[derive(Clone, Debug)]
pub struct EnumMembers {
    members: Vec<EnumMember>,
}

impl EnumMembers {
    /// Retains already acquired evidence without reading references or selecting successors.
    pub fn new(members: Vec<EnumMember>) -> Self {
        Self { members }
    }

    /// Reads the enum's effective `Variants`; the kernel inheritance table keeps them local, so
    /// membership is not inherited.
    pub fn resolve(descriptor: &EnumValueDescriptor) -> Result<Self, HolonError> {
        let source = descriptor.holon();
        let context = source.transaction_context();
        let targets = effective_relationship_targets(source, CoreRelationshipTypeName::Variants)?;
        let mut members = Vec::with_capacity(targets.len());
        for target in targets {
            members.push(EnumMember::from_contribution(&target, context.as_ref())??);
        }
        Ok(Self::new(members))
    }

    /// Authored evidence, including abstract targets and repeated identities.
    pub fn members(&self) -> &[EnumMember] {
        &self.members
    }

    /// Abstract targets are definition defects and never contribute tokens.
    pub fn abstract_members(&self) -> impl Iterator<Item = &EnumMember> {
        self.members.iter().filter(|member| member.is_abstract)
    }

    /// Exact, case-sensitive token membership with distinct definition identities.
    pub fn membership(&self, token: &str) -> EnumMembership {
        let mut identities = Vec::new();
        for member in &self.members {
            if !member.is_abstract
                && member.token.0 == token
                && !identities.contains(&member.identity)
            {
                identities.push(member.identity.clone());
            }
        }
        match identities.len() {
            0 => EnumMembership::Undeclared,
            1 => EnumMembership::Member(identities.remove(0)),
            _ => EnumMembership::Ambiguous(identities),
        }
    }

    /// Duplicate concrete tokens in lexical order, retaining every distinct claimant.
    pub fn duplicate_tokens(&self) -> BTreeMap<String, Vec<DefinitionIdentity>> {
        let mut tokens: BTreeMap<String, Vec<DefinitionIdentity>> = BTreeMap::new();
        for member in self.members.iter().filter(|member| !member.is_abstract) {
            let identities = tokens.entry(member.token.0.clone()).or_default();
            if !identities.contains(&member.identity) {
                identities.push(member.identity.clone());
            }
        }
        tokens.retain(|_, identities| identities.len() > 1);
        tokens
    }
}
