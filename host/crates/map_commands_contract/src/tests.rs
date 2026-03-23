use base_types::{BaseValue, MapString};
use core_types::{LocalId, PropertyName};

use crate::{
    CommandDescriptor, HolonAction, MutationClassification, ReadableHolonAction, SpaceCommand,
    TransactionAction, WritableHolonAction,
};

#[test]
fn space_begin_transaction_descriptor() {
    let desc = SpaceCommand::BeginTransaction.descriptor();
    assert_eq!(desc.mutation, MutationClassification::Mutating);
    assert!(!desc.requires_open_tx);
    assert!(!desc.requires_commit_guard);
}

#[test]
fn transaction_action_descriptors() {
    assert_eq!(TransactionAction::Commit.descriptor(), CommandDescriptor::mutating_with_guard());
    assert_eq!(
        TransactionAction::StagedCount.descriptor(),
        CommandDescriptor::transaction_read_only()
    );
    assert_eq!(
        TransactionAction::TransientCount.descriptor(),
        CommandDescriptor::transaction_read_only()
    );
    assert_eq!(
        TransactionAction::GetAllHolons.descriptor(),
        CommandDescriptor::transaction_read_only()
    );
    assert_eq!(
        TransactionAction::NewHolon { key: None }.descriptor(),
        CommandDescriptor::mutating()
    );
    assert_eq!(
        TransactionAction::DeleteHolon {
            local_id: LocalId(vec![]),
        }
        .descriptor(),
        CommandDescriptor::mutating()
    );
}

#[test]
fn holon_action_descriptors() {
    assert_eq!(
        HolonAction::Read(ReadableHolonAction::Key).descriptor(),
        CommandDescriptor::holon_read_only()
    );
    assert_eq!(
        HolonAction::Read(ReadableHolonAction::CloneHolon).descriptor(),
        CommandDescriptor::mutating(),
        "CloneHolon creates a transient — mutating despite being a ReadableHolonAction"
    );
    assert_eq!(
        HolonAction::Write(WritableHolonAction::WithPropertyValue {
            name: PropertyName(MapString::from("x")),
            value: BaseValue::StringValue(MapString::from("v")),
        })
        .descriptor(),
        CommandDescriptor::mutating()
    );
}
