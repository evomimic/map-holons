//! Additional creation workloads using only definitions already loaded by bootstrap.
use holons_core::RelationshipReadHint;
use holons_prelude::prelude::*;
use map_commands_contract::{
    MapCommand, MapResult, SpaceCommand, TransactionAction, TransactionCommand,
};
use map_commands_runtime::{ExecutionPolicy, Runtime};
use std::{sync::Arc, time::Instant};
use tracing::info;

#[derive(Clone, Copy)]
pub enum CreationWorkload {
    VisualizerSlot,
    Descriptor,
}

fn lookup(context: &Arc<TransactionContext>, key: &str) -> HolonReference {
    context.lookup().get_saved_holon_by_key(&MapString::from(key)).unwrap().into()
}

/// The saved Schema that owns the HolonType root, reached through `ComponentOf` so the
/// workload does not depend on the Core Schema's release key.
fn holon_type_schema(context: &Arc<TransactionContext>) -> HolonReference {
    let owners = lookup(context, "HolonType.TypeDescriptor")
        .related_holons("ComponentOf")
        .unwrap()
        .read()
        .unwrap()
        .get_members()
        .clone();
    assert_eq!(owners.len(), 1, "HolonType.TypeDescriptor must have exactly one owning Schema");
    owners[0].clone()
}

/// A second ordinary instance type, with a native string property and one authored
/// relationship besides DescribedBy. Staging supplies OwnedBy as in the usage workload.
pub fn stage_visualizer_slot(context: &Arc<TransactionContext>, key: &str) -> StagedReference {
    let mut slot = context.mutation().new_holon(Some(key.into())).unwrap();
    slot.with_property_value("VisualizerSlotDescription", "Commit performance slot").unwrap();
    slot.add_related_holons("DescribedBy", vec![lookup(context, "VisualizerSlot.HolonType")])
        .unwrap();
    slot.add_related_holons(
        "AcceptsVisualizerType",
        vec![lookup(context, "PropertyMapVisualizer.HolonType")],
    )
    .unwrap();
    context.mutation().stage_new_holon(slot).unwrap()
}

fn stage_descriptor(context: &Arc<TransactionContext>, key: &str) -> StagedReference {
    let mut descriptor = context.mutation().new_holon(Some(key.into())).unwrap();
    for (name, value) in [
        ("TypeName", key.to_base_value()),
        ("TypeNamePlural", format!("{key}s").to_base_value()),
        ("DisplayName", "Commit performance descriptor".to_base_value()),
        ("DisplayNamePlural", "Commit performance descriptors".to_base_value()),
        ("Description", "Minimal descriptor creation workload".to_base_value()),
        ("DefinesInstanceTypeKind", false.to_base_value()),
        ("IsAbstractType", false.to_base_value()),
        ("InstanceDeletionAllowed", true.to_base_value()),
    ] {
        descriptor.with_property_value(name, value).unwrap();
    }
    for (name, target) in [
        ("Extends", lookup(context, "HolonType.TypeDescriptor")),
        ("ComponentOf", holon_type_schema(context)),
        ("DescribedBy", lookup(context, "MetaHolonType.MetaTypeDescriptor")),
    ] {
        descriptor.add_related_holons(name, vec![target]).unwrap();
    }
    context.mutation().stage_new_holon(descriptor).unwrap()
}

pub async fn run(runtime: &Runtime, samples: usize, subjects: usize, workload: CreationWorkload) {
    let (mode, descriptor_key, outbound_members) = match workload {
        CreationWorkload::VisualizerSlot => ("visualizer-slot", "VisualizerSlot.HolonType", 3),
        CreationWorkload::Descriptor => {
            assert_eq!(
                subjects, 1,
                "descriptor samples create one descriptor and affect one Schema"
            );
            ("descriptor", "MetaHolonType.MetaTypeDescriptor", 4)
        }
    };
    for sample in 0..samples {
        let MapResult::TransactionCreated { tx_id } = runtime
            .execute_command(
                MapCommand::Space(SpaceCommand::BeginTransaction),
                ExecutionPolicy::default(),
            )
            .await
            .unwrap()
        else {
            panic!("BeginTransaction must return a transaction");
        };
        let context = runtime.session().get_transaction(&tx_id).unwrap();
        let descriptor = lookup(&context, descriptor_key);
        let owner = context.get_space_holon().unwrap().unwrap();
        let mut expected_edges =
            vec![("DescribedBy", "Instances", descriptor), ("OwnedBy", "Owns", owner)];
        match workload {
            CreationWorkload::VisualizerSlot => expected_edges.push((
                "AcceptsVisualizerType",
                "AcceptedByVisualizerSlot",
                lookup(&context, "PropertyMapVisualizer.HolonType"),
            )),
            CreationWorkload::Descriptor => expected_edges.extend([
                ("Extends", "ExtendedBy", lookup(&context, "HolonType.TypeDescriptor")),
                ("ComponentOf", "Components", holon_type_schema(&context)),
            ]),
        }
        for subject_index in 0..subjects {
            let key = format!("commit-baseline-{mode}-{sample}-{subject_index}");
            let staged = match workload {
                CreationWorkload::VisualizerSlot => stage_visualizer_slot(&context, &key),
                CreationWorkload::Descriptor => stage_descriptor(&context, &key),
            };
            assert_eq!(
                HolonReference::from(&staged)
                    .all_related_holons()
                    .unwrap()
                    .iter()
                    .into_iter()
                    .map(|(_, members)| members.read().unwrap().get_members().len())
                    .sum::<usize>(),
                outbound_members
            );
        }
        // Only the Commit command is timed. Every sample uses a fresh transaction;
        // inverse membership grows across repeated samples, as in the usage workload.
        let condition = if sample == 0 { "first" } else { "repeated" };
        info!("[commit-baseline] sample_start={} condition={} candidates={} mode={} outbound_members={} prior_creations={} affected_schemas={}",
            sample, condition, subjects, mode, subjects * outbound_members, sample * subjects,
            usize::from(matches!(workload, CreationWorkload::Descriptor)));
        let started = Instant::now();
        let response = runtime
            .execute_command(
                MapCommand::Transaction(TransactionCommand {
                    context: context.clone(),
                    action: TransactionAction::Commit,
                }),
                ExecutionPolicy::default(),
            )
            .await
            .unwrap();
        let command_us = started.elapsed().as_micros();
        let MapResult::Reference(response) = response else {
            panic!("Commit must return a response");
        };
        assert_eq!(
            response.property_value("CommitRequestStatus").unwrap(),
            Some("Complete".to_base_value())
        );
        assert_eq!(
            response.property_value("CommitsAttempted").unwrap(),
            Some((subjects as i64).to_base_value())
        );
        assert_eq!(
            response.property_value("ValidationViolationCount").unwrap(),
            Some(0_i64.to_base_value())
        );
        let saved =
            response.related_holons("SavedHolons").unwrap().read().unwrap().get_members().clone();
        assert_eq!(saved.len(), subjects);
        info!(
            "[commit-baseline] sample_end={} condition={} command_us={}",
            sample, condition, command_us
        );

        // Check persisted forward and inverse membership outside the command interval.
        for saved in saved {
            if matches!(workload, CreationWorkload::VisualizerSlot) {
                assert_eq!(
                    saved.property_value("VisualizerSlotDescription").unwrap(),
                    Some("Commit performance slot".to_base_value())
                );
            }
            for (forward, inverse, target) in &expected_edges {
                let members = saved
                    .related_holons_with_hint(*forward, RelationshipReadHint::RequireFresh)
                    .unwrap();
                assert_eq!(
                    members
                        .read()
                        .unwrap()
                        .get_members()
                        .iter()
                        .map(|member| member.holon_id().unwrap())
                        .collect::<Vec<_>>(),
                    vec![target.holon_id().unwrap()]
                );
                let members = target
                    .related_holons_with_hint(*inverse, RelationshipReadHint::RequireFresh)
                    .unwrap();
                assert_eq!(
                    members
                        .read()
                        .unwrap()
                        .get_members()
                        .iter()
                        .filter(|member| member.holon_id().unwrap() == saved.holon_id().unwrap())
                        .count(),
                    1
                );
            }
        }
        runtime.session().archive_transaction(&tx_id).unwrap();
    }
}
