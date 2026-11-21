//! Owner-directed composition resolved in one authoritative execution round.
use super::candidates::{members, one, select_with_reason};
use super::contract::{VisualizerKind, VisualizerOwner, VisualizerSelectionRequest};
use super::{
    composition_slots, CompositionActivation, CompositionSubject, PreparedSelectionMetadata,
};
use crate::core_shared_objects::transactions::TransactionContext;
use crate::descriptors::Descriptor;
use crate::{HolonReference, ReadableHolon, WritableHolon};
use base_types::BaseValue;
use core_types::HolonError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use type_names::{
    CoreHolonTypeName, CorePropertyTypeName, CoreRelationshipTypeName as C,
    DahnPropertyTypeName as P, DahnRelationshipTypeName as R, ToPropertyName, ToRelationshipName,
};

/// A request carries identities, never authority or a frontend expansion recipe.
#[derive(Debug, Clone)]
pub struct CompositionRequest {
    pub owner: HolonReference,
    pub subject: HolonReference,
    pub theme: HolonReference,
    pub demand_slot: Option<HolonReference>,
    pub collection_source: Option<HolonReference>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionReason {
    PersonalPreference,
    PrescribedDefault,
    Discovery,
}
/// Indexes refer to the shared exact-reference table, not serialized runtime handles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChildBinding {
    pub parent: Option<usize>,
    pub owner: usize,
    pub slot: usize,
    pub subject: usize,
    pub subject_descriptor: usize,
    pub subject_binding: CompositionSubject,
    pub role: String,
    pub property_name: Option<String>,
    pub label: Option<String>,
    pub selected: usize,
    pub implementation: usize,
    pub usage: Option<usize>,
    pub reason: SelectionReason,
    pub governing_record: Option<usize>,
    pub configuration: std::collections::BTreeMap<String, BaseValue>,
    pub required_operators: Vec<String>,
    pub offered_operators: Vec<String>,
}
/// Prepared property vocabulary; subject contents remain outside this result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositionProperty {
    pub subject: usize,
    pub descriptor: usize,
    pub name: String,
    pub label: String,
    pub is_array: bool,
}
/// Scalar metadata is shared by identity, including configuration and implementation fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositionMetadata {
    pub reference: usize,
    pub key: Option<String>,
    pub identity: String,
    pub versioned_key: Option<String>,
    pub properties: serde_json::Value,
}
/// An observed complete membership set. An empty set is evidence of absence,
/// never a failed lookup. Exact references retain the observed revisions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositionPopulation {
    pub source: usize,
    pub relationship: String,
    pub members: Vec<usize>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CompositionWork {
    pub selections: usize,
    pub binding_reuses: usize,
    pub metadata_reuses: usize,
    pub personal_population_requests: usize,
    pub candidate_population_requests: usize,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CompositionData {
    #[serde(default)]
    pub subject_properties: Vec<CompositionProperty>,
    pub bindings: Vec<ChildBinding>,
    pub populations: Vec<CompositionPopulation>,
    pub theme: Option<usize>,
    pub work: CompositionWork,
    pub metadata: Vec<CompositionMetadata>,
    /// Reuse is bounded to this authoritative round. No prior round, including a
    /// confirmed preference absence, authorizes reuse after a save or context change.
    pub reusable_across_rounds: bool,
}
#[derive(Debug)]
pub struct ResolvedComposition {
    pub references: Vec<HolonReference>,
    pub data: CompositionData,
}
struct Resolver<'a> {
    context: &'a Arc<TransactionContext>,
    theme: HolonReference,
    result: ResolvedComposition,
    selection: PreparedSelectionMetadata,
    properties: Vec<(HolonReference, Vec<(HolonReference, String, String, bool)>)>,
    winners: Vec<(
        HolonReference,
        HolonReference,
        HolonReference,
        super::contract::VisualizerSelection,
        SelectionReason,
        Option<HolonReference>,
    )>,
}
impl Resolver<'_> {
    fn identity(&mut self, reference: &HolonReference) -> usize {
        if let Some(index) = self.result.references.iter().position(|r| r == reference) {
            return index;
        }
        let index = self.result.references.len();
        self.result.references.push(reference.clone());
        index
    }
    fn reference(&mut self, reference: &HolonReference) -> Result<usize, HolonError> {
        let index = self.identity(reference);
        if self.result.data.metadata.iter().any(|m| m.reference == index) {
            return Ok(index);
        }
        self.result.data.metadata.push(CompositionMetadata {
            reference: index,
            key: reference.key()?.map(|v| v.0),
            identity: reference.full_reference_id_string(),
            versioned_key: if reference.key()?.is_some() {
                Some(reference.versioned_key()?.0)
            } else {
                None
            },
            properties: serde_json::to_value(reference.clone().into_model()?.property_map)
                .map_err(|e| HolonError::InvalidParameter(e.to_string()))?,
        });
        Ok(index)
    }
    fn properties(
        &mut self,
        subject: &HolonReference,
    ) -> Result<Vec<(HolonReference, String, String, bool)>, HolonError> {
        if let Some((_, properties)) = self.properties.iter().find(|(owner, _)| owner == subject) {
            return Ok(properties.clone());
        }
        let schema = subject.holon_descriptor()?.effective_descriptor()?;
        let mut properties = Vec::new();
        for property in schema.instance_properties()? {
            let name = property.property_name()?.to_string();
            let label =
                property.header().display_name()?.map(|v| v.0).unwrap_or_else(|| name.clone());
            let is_array = property.value_type()?.is_array()?;
            let entry = CompositionProperty {
                subject: self.identity(subject),
                descriptor: self.reference(property.holon())?,
                name: name.clone(),
                label: label.clone(),
                is_array,
            };
            self.result.data.subject_properties.push(entry);
            properties.push((property.holon().clone(), name, label, is_array));
        }
        self.properties.push((subject.clone(), properties.clone()));
        Ok(properties)
    }
    fn expand(
        &mut self,
        owner: &HolonReference,
        subject: &HolonReference,
        parent: Option<usize>,
        demand: Option<&HolonReference>,
        collection: Option<&HolonReference>,
        ancestors: &[HolonReference],
    ) -> Result<(), HolonError> {
        if ancestors.contains(owner) {
            return Err(HolonError::InvalidState("Cyclic Presentation composition".into()));
        }
        let mut ancestors = ancestors.to_vec();
        ancestors.push(owner.clone());
        let slots = composition_slots(owner).map_err(|e| phase("reading owner Slots", e))?;
        if demand.is_some_and(|slot| !slots.contains(slot)) {
            return Err(HolonError::InvalidParameter(
                "Requested Slot does not belong to the selected owner".into(),
            ));
        }
        for slot in slots {
            let activation = CompositionActivation::read(&slot)?;
            if let Some(demand) = demand {
                if demand != &slot {
                    continue;
                }
            } else if activation != CompositionActivation::Presentation {
                continue;
            }
            let binding = CompositionSubject::read(&slot)?;
            let role = text(&slot, P::CompositionRole)?;
            let kind = kind(&slot)?;
            let mut subjects = Vec::new();
            match binding {
                CompositionSubject::OwnerSubject => {
                    subjects.push((subject.clone(), subject.clone(), None, None))
                }
                CompositionSubject::ScalarPropertyLabel
                | CompositionSubject::ScalarPropertyValue => {
                    for (property, name, label, is_array) in self
                        .properties(subject)
                        .map_err(|e| phase("reading property vocabulary", e))?
                    {
                        if is_array {
                            continue;
                        }
                        let selection_subject =
                            if binding == CompositionSubject::ScalarPropertyLabel {
                                self.context
                                    .lookup()
                                    .get_saved_holon_by_key(
                                        &format!(
                                            "{}.PropertyType",
                                            CorePropertyTypeName::DisplayName.to_property_name()
                                        )
                                        .into(),
                                    )?
                                    .into()
                            } else {
                                property.clone()
                            };
                        subjects.push((property, selection_subject, Some(name), Some(label)));
                    }
                }
                CompositionSubject::AffordedAction => {
                    for dance in
                        subject.holon_descriptor()?.effective_descriptor()?.afforded_dances()?
                    {
                        subjects.push((dance.holon().clone(), dance.holon().clone(), None, None));
                    }
                }
                CompositionSubject::SelectedCollection => {
                    let source = collection
                        .cloned()
                        .or(super::defaults::optional(&slot, R::InitialCollectionSource)?);
                    let source = source.ok_or_else(|| {
                        HolonError::InvalidParameter(
                            "SelectedCollection requires an explicit collection source".into(),
                        )
                    })?;
                    // Validate the source against the owner's bound subject without reading members.
                    let relationships = subject
                        .holon_descriptor()?
                        .effective_descriptor()?
                        .available_relationships(subject)?;
                    if !relationships.iter().any(|r| r.holon() == &source) {
                        return Err(HolonError::InvalidParameter(
                            "Collection source is not available on the bound subject".into(),
                        ));
                    }
                    // The source identifies a collection, not one of its elements.
                    // A typed transient witness permits semantic selection without reading members.
                    let collection_type = self.context.lookup().get_saved_holon_by_key(
                        &CoreHolonTypeName::HolonCollection
                            .descriptor_key(&CoreHolonTypeName::HolonType),
                    )?;
                    let mut witness = self.context.mutation().new_holon(None)?;
                    witness.add_related_holons(C::DescribedBy, vec![collection_type.into()])?;
                    subjects.push((source, witness.into(), None, None));
                }
            }
            for (bound_subject, selection_subject, property_name, label) in subjects {
                let request = VisualizerSelectionRequest {
                    subject: selection_subject,
                    owner: VisualizerOwner::Visualizer(owner.clone()),
                    slot: slot.clone(),
                    theme: self.theme.clone(),
                    requested_kind: kind,
                };
                let descriptor = super::usage_subject_type(&request)
                    .map_err(|e| phase(&format!("binding {role} subject"), e))?;
                let (selected, reason, governing_record) =
                    if let Some((_, _, _, selection, reason, record)) = self.winners.iter().find(
                        |(candidate_owner, candidate_slot, candidate_type, _, _, _)| {
                            candidate_owner == owner
                                && candidate_slot == &slot
                                && candidate_type == &descriptor
                        },
                    ) {
                        self.result.data.work.binding_reuses += 1;
                        (selection.clone(), *reason, record.clone())
                    } else {
                        self.result.data.work.selections += 1;
                        let (selection, _, reason, record) =
                            select_with_reason(self.context, request, Some(&mut self.selection))
                                .map_err(|e| phase(&format!("selecting {role}"), e))?;
                        self.winners.push((
                            owner.clone(),
                            slot.clone(),
                            descriptor.clone(),
                            selection.clone(),
                            reason,
                            record.clone(),
                        ));
                        (selection, reason, record)
                    };
                let implementation = one(&selected.selected, "ImplementedBy")?;
                let required_operators = operator_keys(&slot, "RequiresOperator")?;
                let offered_operators = operator_keys(&selected.selected, "OffersOperator")?;
                let child = ChildBinding {
                    parent,
                    owner: self.reference(owner)?,
                    slot: self.reference(&slot)?,
                    subject: if binding == CompositionSubject::OwnerSubject {
                        self.identity(&bound_subject)
                    } else {
                        self.reference(&bound_subject)?
                    },
                    subject_descriptor: self.reference(&descriptor)?,
                    subject_binding: binding,
                    role: role.clone(),
                    property_name,
                    label,
                    selected: self.reference(&selected.selected)?,
                    implementation: self.reference(&implementation)?,
                    usage: selected.usage.as_ref().map(|u| self.reference(&u.usage)).transpose()?,
                    reason,
                    governing_record: governing_record.as_ref().map(|r| self.identity(r)),
                    configuration: effective_configuration(
                        &selected.selected,
                        selected.usage.as_ref().map(|u| &u.usage),
                    )
                    .map_err(|e| phase(&format!("loading {role} configuration"), e))?,
                    required_operators,
                    offered_operators,
                };
                let index = self.result.data.bindings.len();
                self.result.data.bindings.push(child);
                self.expand(
                    &selected.selected,
                    &bound_subject,
                    Some(index),
                    None,
                    None,
                    &ancestors,
                )
                .map_err(|e| phase(&format!("expanding {role}"), e))?;
            }
        }
        Ok(())
    }
}
/// Expands selected-owner declarations once. No executable artifacts or subject
/// values/populations are read merely to choose a renderer.
pub fn resolve(
    context: &Arc<TransactionContext>,
    request: CompositionRequest,
) -> Result<ResolvedComposition, HolonError> {
    let mut resolver = Resolver {
        context,
        theme: request.theme,
        result: ResolvedComposition { references: vec![], data: CompositionData::default() },
        selection: PreparedSelectionMetadata::default(),
        properties: Vec::new(),
        winners: Vec::new(),
    };
    resolver
        .expand(
            &request.owner,
            &request.subject,
            None,
            request.demand_slot.as_ref(),
            request.collection_source.as_ref(),
            &[],
        )
        .map_err(|error| {
            HolonError::InvalidState(format!("Resolving owner-directed composition: {error}"))
        })?;
    resolver.result.data.theme = Some(resolver.reference(&resolver.theme.clone())?);
    for (source, relationship, members) in resolver.selection.observed_populations() {
        let source = resolver.identity(&source);
        let members = members.iter().map(|member| resolver.identity(member)).collect();
        resolver.result.data.populations.push(CompositionPopulation {
            source,
            relationship,
            members,
        });
    }
    resolver.result.data.work.metadata_reuses = resolver.selection.hits();
    (
        resolver.result.data.work.personal_population_requests,
        resolver.result.data.work.candidate_population_requests,
    ) = resolver.selection.population_requests();
    Ok(resolver.result)
}
fn text(source: &HolonReference, property: P) -> Result<String, HolonError> {
    match source.property_value(property.to_property_name())? {
        Some(BaseValue::StringValue(value)) => Ok(value.0),
        _ => Err(HolonError::InvalidParameter(format!("Missing {property:?}"))),
    }
}
fn kind(slot: &HolonReference) -> Result<VisualizerKind, HolonError> {
    match slot.property_value(P::VisualizerKind.to_property_name())? {
        Some(BaseValue::EnumValue(value)) => match value.to_string().as_str() {
            "TitleBar" => Ok(VisualizerKind::TitleBar),
            "Canvas" => Ok(VisualizerKind::Canvas),
            "Node" => Ok(VisualizerKind::Node),
            "Structure" => Ok(VisualizerKind::Structure),
            "RootedNavigation" => Ok(VisualizerKind::RootedNavigation),
            "Collection" => Ok(VisualizerKind::Collection),
            "PropertyMap" => Ok(VisualizerKind::PropertyMap),
            "Value" => Ok(VisualizerKind::Value),
            "ActionBar" => Ok(VisualizerKind::ActionBar),
            "Action" => Ok(VisualizerKind::Action),
            other => Err(HolonError::InvalidParameter(format!("Unknown VisualizerKind {other}"))),
        },
        _ => Err(HolonError::InvalidParameter("Missing enum VisualizerKind".into())),
    }
}
fn effective_configuration(
    visualizer: &HolonReference,
    usage: Option<&HolonReference>,
) -> Result<std::collections::BTreeMap<String, BaseValue>, HolonError> {
    let schema =
        visualizer.effective_descriptor().map_err(|e| phase("acquiring Visualizer schema", e))?;
    let relationship = schema
        .get_relationship_by_name("UsedByVisualizerUsage")
        .map_err(|e| phase("reading Usage contract", e))?;
    let usage_type =
        relationship.target_type().map_err(|e| phase("reading Usage type", e))?.holon().clone();
    let descriptor = if let Some(usage) = usage {
        usage.effective_descriptor().map_err(|e| phase("acquiring configured Usage schema", e))?
    } else {
        crate::descriptors::HolonDescriptor::from_holon(usage_type)
            .effective_descriptor()
            .map_err(|e| phase("acquiring default Usage schema", e))?
    };
    let mut values = std::collections::BTreeMap::new();
    for property in
        descriptor.instance_properties().map_err(|e| phase("reading configuration fields", e))?
    {
        let name = property.property_name()?;
        let value = if let Some(usage) = usage {
            property.effective_value(usage)?
        } else {
            crate::descriptors::effective_property_value(
                property.holon(),
                type_names::CorePropertyTypeName::DefaultValue,
            )?
            .map(crate::descriptors::EffectiveValue::Default)
        };
        if let Some(
            crate::descriptors::EffectiveValue::Authored(value)
            | crate::descriptors::EffectiveValue::Default(value),
        ) = value
        {
            values.insert(name.to_string(), value);
        }
    }
    Ok(values)
}
fn operator_keys(source: &HolonReference, name: &str) -> Result<Vec<String>, HolonError> {
    members(source, name)?
        .iter()
        .map(|r| {
            r.key()?
                .map(|v| v.0)
                .ok_or_else(|| HolonError::InvalidState("Operator has no key".into()))
        })
        .collect()
}
impl ResolvedComposition {
    /// Scalar payload and one ordered reference collection preserve boundary semantics.
    pub fn into_projection(
        self,
        context: &Arc<TransactionContext>,
    ) -> Result<HolonReference, HolonError> {
        let descriptor = context.lookup().get_saved_holon_by_key(
            &type_names::DahnHolonTypeName::ResolvedComposition.descriptor_key(),
        )?;
        let mut body: HolonReference = context.mutation().new_holon(None)?.into();
        body.add_related_holons(C::DescribedBy, vec![descriptor.into()])?;
        body.with_property_value(
            P::CompositionData.to_property_name(),
            serde_json::to_string(&self.data)
                .map_err(|e| HolonError::InvalidParameter(e.to_string()))?,
        )?;
        if !self.references.is_empty() {
            body.add_related_holons(
                R::CompositionReferences.to_relationship_name(),
                self.references,
            )?;
        }
        Ok(body)
    }
    pub fn from_projection(body: &HolonReference) -> Result<Self, HolonError> {
        if !matches!(body, HolonReference::Transient(_)) {
            return Err(HolonError::InvalidParameter("Composition body must be transient".into()));
        }
        let data: CompositionData = serde_json::from_str(&text(body, P::CompositionData)?)
            .map_err(|e| HolonError::InvalidParameter(e.to_string()))?;
        let references =
            members(body, &R::CompositionReferences.to_relationship_name().to_string())?;
        let length = references.len();
        let metadata =
            data.metadata.iter().map(|m| m.reference).collect::<std::collections::HashSet<_>>();
        if metadata.len() != data.metadata.len()
            || metadata.iter().any(|index| *index >= length)
            || data
                .subject_properties
                .iter()
                .any(|p| p.subject >= length || !metadata.contains(&p.descriptor))
            || data.bindings.iter().enumerate().any(|(i, b)| {
                b.parent.is_some_and(|p| p >= i)
                    || [
                        b.owner,
                        b.slot,
                        b.subject,
                        b.subject_descriptor,
                        b.selected,
                        b.implementation,
                    ]
                    .iter()
                    .any(|v| *v >= length)
                    || b.usage.is_some_and(|v| v >= length)
                    || [b.owner, b.slot, b.subject_descriptor, b.selected, b.implementation]
                        .iter()
                        .any(|index| !metadata.contains(index))
                    || b.usage.is_some_and(|index| !metadata.contains(&index))
            })
            || data
                .populations
                .iter()
                .any(|p| p.source >= length || p.members.iter().any(|m| *m >= length))
            || data.theme.is_none_or(|index| index >= length)
            || data.bindings.iter().any(|b| b.governing_record.is_some_and(|i| i >= length))
            || data.reusable_across_rounds
        {
            return Err(HolonError::InvalidParameter("Malformed composition projection".into()));
        }
        Ok(Self { references, data })
    }
}

fn phase(phase: &str, error: HolonError) -> HolonError {
    HolonError::InvalidState(format!("{phase}: {error}"))
}
