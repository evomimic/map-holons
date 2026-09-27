//! Classification-independent structural products. No validation dispatch occurs here.
use super::{
    accessor_helpers::lock_error,
    definition_identity::{lineage_contains, same_definition},
    walk_extends_chain_with_reader, CurrentDescriptorReader, DescriptorReader,
};
use crate::reference_layer::{HolonReference, ReadableHolon};
use core_types::HolonError;
use type_names::CoreRelationshipTypeName;

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
    resolve_describing_type_with_reader(subject, &CurrentDescriptorReader)
}

/// Reads direct describing targets through the assessment's replacement selection.
pub fn resolve_describing_type_with_reader<R: DescriptorReader>(
    subject: &HolonReference,
    reader: &R,
) -> Result<DescribingTypeResolution, R::Error> {
    let targets =
        local_targets_with_reader(subject, CoreRelationshipTypeName::DescribedBy, reader)?;
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
    local_targets_with_reader(subject, relationship, &CurrentDescriptorReader)
}

pub(super) fn local_targets_with_reader<R: DescriptorReader>(
    subject: &HolonReference,
    relationship: CoreRelationshipTypeName,
    reader: &R,
) -> Result<Vec<HolonReference>, R::Error> {
    let subject = reader.select(subject)?;
    let collection = subject.related_holons(relationship)?;
    let members = collection.read().map_err(lock_error)?.get_members().clone();
    members.iter().map(|member| reader.select(member)).collect()
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
    pub defects: Vec<ExtendsLineageDefect>,
}

/// A lineage whose structural checks have completed without a defect.
/// Created only from an [`ExtendsLineageDiagnosis`].
#[derive(Clone, Copy, Debug)]
pub struct ValidExtendsLineage<'a> {
    members: &'a [HolonReference],
}

impl ValidExtendsLineage<'_> {
    /// Returns members in self-first order.
    pub fn members(&self) -> &[HolonReference] {
        self.members
    }

    /// Returns the subject at the start of this lineage.
    pub fn subject(&self) -> &HolonReference {
        &self.members[0]
    }
}

impl ExtendsLineageDiagnosis {
    /// Applies to any holon, without requiring descriptor classification.
    /// The designated root is supplied by the assessment's resolved identities.
    pub fn assess(subject: &HolonReference, root: &HolonReference) -> Result<Self, HolonError> {
        Self::assess_with_reader(subject, root, &CurrentDescriptorReader)
    }

    /// Diagnoses selected prospective content using the existing kernel lineage walk.
    pub fn assess_with_reader<R: DescriptorReader>(
        subject: &HolonReference,
        root: &HolonReference,
        reader: &R,
    ) -> Result<Self, R::Error> {
        let root = reader.select(root)?;
        let mut lineage = Vec::new();
        let mut defects = Vec::new();
        for step in walk_extends_chain_with_reader(subject, reader) {
            match step {
                Ok(holon) => lineage.push(holon),
                Err(error) => match R::operational_error(&error) {
                    Some(HolonError::MultipleExtends { count, .. }) => {
                        defects.push(ExtendsLineageDefect::MultipleParents {
                            subject: lineage
                                .last()
                                .expect("iterator yields subject before defect")
                                .clone(),
                            count: *count,
                        })
                    }
                    Some(HolonError::CyclicExtends { descriptor }) => {
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
        if lineage_contains(&lineage, &root)
            && !local_targets_with_reader(&root, CoreRelationshipTypeName::Extends, reader)?
                .is_empty()
        {
            defects.push(ExtendsLineageDefect::RootHasParent);
        }
        if completed && lineage.len() > 1 && !same_definition(lineage.last().unwrap(), &root) {
            defects.push(ExtendsLineageDefect::WrongTermination {
                terminal: lineage.last().unwrap().clone(),
            });
        }
        Ok(Self { lineage, defects })
    }

    /// Exposes the lineage to kind checks only when diagnosis found no defect.
    pub fn valid_lineage(&self) -> Option<ValidExtendsLineage<'_>> {
        self.defects.is_empty().then_some(ValidExtendsLineage { members: &self.lineage })
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
    pub fn assess(subject: &HolonReference, root: &HolonReference) -> Result<Self, HolonError> {
        Self::assess_with_reader(subject, root, &CurrentDescriptorReader)
    }

    /// Diagnoses selected prospective content using the existing kernel lineage walk.
    pub fn assess_with_reader<R: DescriptorReader>(
        subject: &HolonReference,
        root: &HolonReference,
        reader: &R,
    ) -> Result<Self, R::Error> {
        let describing_type = resolve_describing_type_with_reader(subject, reader)?;
        let subject_lineage = ExtendsLineageDiagnosis::assess_with_reader(subject, root, reader)?;
        let governing_lineage = match &describing_type {
            DescribingTypeResolution::Unique(descriptor)
                if same_definition(descriptor, &subject_lineage.lineage[0]) =>
            {
                Some(subject_lineage.clone())
            }
            DescribingTypeResolution::Unique(descriptor) => {
                Some(ExtendsLineageDiagnosis::assess_with_reader(descriptor, root, reader)?)
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
