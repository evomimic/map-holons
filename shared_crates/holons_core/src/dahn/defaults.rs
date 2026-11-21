//! Slot-owned prescriptions are recommendations, distinct from applicability and personal intent.
use super::candidates::{assess, members, one, AssessmentContext};
use super::contract::VisualizerAssessment;
use crate::core_shared_objects::transactions::TransactionContext;
use crate::descriptors::walk_extends_chain;
use crate::HolonReference;
use core_types::HolonError;
use std::sync::Arc;
use type_names::{DahnRelationshipTypeName as R, ToRelationshipName};

#[derive(Clone, Debug)]
struct Declaration {
    source: HolonReference,
    condition: Option<HolonReference>,
    visualizer: HolonReference,
}

/// One complete, validated declaration population. Empty is a confirmed declaration miss.
#[derive(Clone, Debug)]
pub struct SlotDefaults {
    slot: HolonReference,
    declarations: Vec<Declaration>,
}
impl SlotDefaults {
    pub fn load(slot: &HolonReference) -> Result<Self, HolonError> {
        let mut declarations: Vec<Declaration> = Vec::new();
        for source in related(slot, R::HasDefaultDeclaration)? {
            if one(&source, &R::DefaultForSlot.to_relationship_name().to_string())? != *slot {
                return Err(HolonError::InvalidState(
                    "DefaultForSlot disagrees with its inverse".into(),
                ));
            }
            let condition = optional(&source, R::DefaultForSubjectType)?;
            if declarations.iter().any(|d| d.condition == condition) {
                return Err(HolonError::InvalidState(format!(
                    "Conflicting default declarations for Slot {slot:?}, condition {condition:?}"
                )));
            }
            let visualizer =
                one(&source, &R::RecommendsVisualizer.to_relationship_name().to_string())?;
            declarations.push(Declaration { source, condition, visualizer });
        }
        Ok(Self { slot: slot.clone(), declarations })
    }

    pub(crate) fn declaration_references(&self) -> Vec<HolonReference> {
        self.declarations.iter().map(|d| d.source.clone()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }

    pub(crate) fn resolve(
        &self,
        context: &Arc<TransactionContext>,
        facts: &AssessmentContext,
    ) -> Result<Option<(HolonReference, HolonReference)>, HolonError> {
        if self.slot != facts.slot {
            return Err(HolonError::InvalidParameter(
                "Default index belongs to a different Slot".into(),
            ));
        }
        if self.declarations.is_empty() {
            return Ok(None);
        }
        let mut declaration = None;
        if self.declarations.iter().any(|d| d.condition.is_some()) {
            for ty in walk_extends_chain(&facts.start) {
                let ty = ty?;
                if let Some(found) =
                    self.declarations.iter().find(|d| d.condition.as_ref() == Some(&ty))
                {
                    declaration = Some(found);
                    break;
                }
            }
        }
        let declaration =
            declaration.or_else(|| self.declarations.iter().find(|d| d.condition.is_none()));
        let Some(declaration) = declaration else {
            return Ok(None);
        };
        let applicable = applicable(&facts.start, &declaration.visualizer)?;
        let assessment = assess(context, facts, &declaration.visualizer)?;
        if !applicable || assessment != VisualizerAssessment::Viable {
            return Err(HolonError::InvalidState(format!("Unusable default: Slot {:?}, declaration {:?}, condition {:?}, Visualizer {:?}, applicability={}, assessment={assessment:?}", self.slot, declaration.source, declaration.condition, declaration.visualizer, applicable)));
        }
        Ok(Some((declaration.visualizer.clone(), declaration.source.clone())))
    }
}

pub(crate) fn related(source: &HolonReference, name: R) -> Result<Vec<HolonReference>, HolonError> {
    members(source, &name.to_relationship_name().to_string())
}
pub(crate) fn optional(
    source: &HolonReference,
    name: R,
) -> Result<Option<HolonReference>, HolonError> {
    match related(source, name)?.as_slice() {
        [] => Ok(None),
        [value] => Ok(Some(value.clone())),
        _ => Err(HolonError::InvalidState(format!("Multiple {name:?} targets"))),
    }
}

pub(crate) fn applicable(
    subject_type: &HolonReference,
    visualizer: &HolonReference,
) -> Result<bool, HolonError> {
    let declarations = related(visualizer, R::ApplicableToType)?;
    if declarations.is_empty() {
        return Ok(false);
    }
    for ty in walk_extends_chain(subject_type) {
        if declarations.contains(&ty?) {
            return Ok(true);
        }
    }
    Ok(false)
}
