//! Classification-independent structural products. No validation dispatch occurs here.
use super::{
    accessor_helpers::lock_error,
    core_roles::record_key,
    definition_identity::{lineage_contains, same_definition},
    walk_extends_chain,
};
use crate::reference_layer::{HolonReference, ReadableHolon};
use base_types::MapString;
use core_types::HolonError;
use type_names::{CoreDefinitionRole, CoreRelationshipTypeName};

/// Direct describing targets, retaining malformed readable state for diagnostics.
#[derive(Clone, Debug)]
pub enum DescribingTypeResolution {
    Missing,
    Unique(HolonReference),
    Multiple(Vec<HolonReference>),
}

/// Reads only the direct describing edge; self-description never recurses.
pub fn resolve_describing_type(
    subject: &HolonReference,
) -> Result<DescribingTypeResolution, HolonError> {
    let targets = local_targets(subject, CoreRelationshipTypeName::DescribedBy)?;
    Ok(match targets.as_slice() {
        [] => DescribingTypeResolution::Missing,
        [one] => DescribingTypeResolution::Unique(one.clone()),
        _ => DescribingTypeResolution::Multiple(targets),
    })
}

pub(super) fn local_targets(
    subject: &HolonReference,
    relationship: CoreRelationshipTypeName,
) -> Result<Vec<HolonReference>, HolonError> {
    let collection = subject.related_holons(relationship)?;
    let members = collection.read().map_err(lock_error)?.get_members().clone();
    Ok(members)
}

/// Stable graph defects, distinct from failures to read the graph.
#[derive(Clone, Debug)]
pub enum ExtendsLineageDefect {
    MultipleParents { subject: HolonReference, count: usize },
    Cycle { path: Vec<HolonReference>, repeated_descriptor: String },
    WrongTermination { terminal: HolonReference },
    RootHasParent,
}

/// One drained lineage, reusable by prerequisite and bound-rule assessment.
/// A defective lineage is diagnostic evidence, never a valid classification product.
#[derive(Clone, Debug)]
pub struct ExtendsLineageDiagnosis {
    pub lineage: Vec<HolonReference>,
    /// Record keys aligned with the diagnosed lineage, read once during diagnosis.
    pub(crate) member_keys: Vec<Option<MapString>>,
    pub defects: Vec<ExtendsLineageDefect>,
}

/// A lineage whose structural checks have completed without a defect.
/// Created only from an [`ExtendsLineageDiagnosis`].
#[derive(Clone, Copy, Debug)]
pub struct ValidExtendsLineage<'a> {
    members: &'a [HolonReference],
    member_keys: &'a [Option<MapString>],
}

impl<'a> ValidExtendsLineage<'a> {
    /// Tests the subject's own diagnosed record key without reading the graph again.
    pub fn subject_has_role(&self, role: CoreDefinitionRole) -> bool {
        self.member_keys[0].as_ref().is_some_and(|key| key.0 == role.key())
    }

    /// Recognizes a Core role without reading the graph again.
    pub fn contains_role(&self, role: CoreDefinitionRole) -> bool {
        self.member_with_role(role).is_some()
    }

    /// Returns the actual reached definition carrying a Core role, in self-first order.
    pub fn member_with_role(&self, role: CoreDefinitionRole) -> Option<&'a HolonReference> {
        self.members.iter().zip(self.member_keys).find_map(|(member, key)| {
            key.as_ref().is_some_and(|key| key.0 == role.key()).then_some(member)
        })
    }

    /// Returns members in self-first order.
    pub fn members(&self) -> &[HolonReference] {
        self.members
    }

    /// Recognizes a role at or above an exact member of this diagnosed lineage.
    pub fn contains_role_from(
        &self,
        start: &HolonReference,
        role: CoreDefinitionRole,
    ) -> Result<bool, HolonError> {
        let mut reached_start = false;
        for (member, key) in self.members.iter().zip(self.member_keys) {
            reached_start |= same_definition(member, start)?;
            if reached_start && key.as_ref().is_some_and(|key| key.0 == role.key()) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Tests exact definition membership without re-reading the anchor for each member.
    pub fn contains(&self, anchor: &HolonReference) -> Result<bool, HolonError> {
        lineage_contains(self.members, anchor)
    }

    /// Returns the subject at the start of this lineage.
    pub fn subject(&self) -> &HolonReference {
        &self.members[0]
    }
}

impl ExtendsLineageDiagnosis {
    /// Applies to any holon, without requiring descriptor classification.
    pub fn assess(subject: &HolonReference) -> Result<Self, HolonError> {
        let mut lineage = Vec::new();
        let mut member_keys = Vec::new();
        let mut defects = Vec::new();
        for step in walk_extends_chain(subject) {
            match step {
                Ok(holon) => {
                    member_keys.push(record_key(&holon)?);
                    lineage.push(holon);
                }
                Err(error) => match &error {
                    HolonError::MultipleExtends { count, .. } => {
                        defects.push(ExtendsLineageDefect::MultipleParents {
                            subject: lineage
                                .last()
                                .expect("iterator yields subject before defect")
                                .clone(),
                            count: *count,
                        })
                    }
                    HolonError::CyclicExtends { descriptor } => {
                        defects.push(ExtendsLineageDefect::Cycle {
                            path: lineage.clone(),
                            repeated_descriptor: descriptor.clone(),
                        })
                    }
                    _ => return Err(error),
                },
            }
        }
        let completed = defects.is_empty();
        // A root reached before a defect must still satisfy its own local invariant.
        for (member, key) in lineage.iter().zip(&member_keys) {
            if key.as_ref().is_some_and(|key| key.0 == CoreDefinitionRole::TypeDescriptor.key())
                && !local_targets(member, CoreRelationshipTypeName::Extends)?.is_empty()
            {
                defects.push(ExtendsLineageDefect::RootHasParent);
                break;
            }
        }
        if completed
            && lineage.len() > 1
            && !member_keys
                .last()
                .unwrap()
                .as_ref()
                .is_some_and(|key| key.0 == CoreDefinitionRole::TypeDescriptor.key())
        {
            defects.push(ExtendsLineageDefect::WrongTermination {
                terminal: lineage.last().unwrap().clone(),
            });
        }
        Ok(Self { lineage, member_keys, defects })
    }

    /// Exposes the lineage to kind checks only when diagnosis found no defect.
    pub fn valid_lineage(&self) -> Option<ValidExtendsLineage<'_>> {
        self.defects.is_empty().then_some(ValidExtendsLineage {
            members: &self.lineage,
            member_keys: &self.member_keys,
        })
    }
}

/// Structural input shared by later prerequisite and binding dispatch paths.
/// This is an assessment result, not a cache: discard it before graph mutation.
#[derive(Clone, Debug)]
pub struct StructuralPrerequisites {
    pub describing_type: DescribingTypeResolution,
    pub subject_lineage: ExtendsLineageDiagnosis,
    pub governing_lineage: Option<ExtendsLineageDiagnosis>,
}
impl StructuralPrerequisites {
    /// Diagnoses the subject and its unique direct describer without classifying either.
    /// Self-description reuses the subject diagnosis instead of recursively validating.
    pub fn assess(subject: &HolonReference) -> Result<Self, HolonError> {
        let describing_type = resolve_describing_type(subject)?;
        let subject_lineage = ExtendsLineageDiagnosis::assess(subject)?;
        let governing_lineage = match &describing_type {
            DescribingTypeResolution::Unique(descriptor)
                if same_definition(descriptor, &subject_lineage.lineage[0])? =>
            {
                Some(subject_lineage.clone())
            }
            DescribingTypeResolution::Unique(descriptor) => {
                Some(ExtendsLineageDiagnosis::assess(descriptor)?)
            }
            _ => None,
        };
        Ok(Self { describing_type, subject_lineage, governing_lineage })
    }

    /// Returns both checked lineages only when direct describing selection is unique.
    pub fn valid_lineages(&self) -> Option<(ValidExtendsLineage<'_>, ValidExtendsLineage<'_>)> {
        Some((
            self.subject_lineage.valid_lineage()?,
            self.governing_lineage.as_ref()?.valid_lineage()?,
        ))
    }
}
