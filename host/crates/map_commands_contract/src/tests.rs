use base_types::{BaseValue, MapString};
use core_types::{LocalId, PropertyName};

use crate::{
    CommandLifecyclePolicy, HolonAction, MutationClassification, ReadableHolonAction, SpaceCommand,
    TransactionAction, WritableHolonAction,
};

#[test]
fn space_begin_transaction_policy() {
    let policy = SpaceCommand::BeginTransaction.policy();
    assert_eq!(policy.mutation, MutationClassification::Mutating);
    assert!(!policy.requires_open_tx);
    assert!(!policy.requires_commit_guard);
}

#[test]
fn transaction_action_policies() {
    assert_eq!(TransactionAction::Commit.policy(), CommandLifecyclePolicy::mutating_with_guard());
    assert_eq!(
        TransactionAction::StagedCount.policy(),
        CommandLifecyclePolicy::transaction_read_only()
    );
    assert_eq!(
        TransactionAction::TransientCount.policy(),
        CommandLifecyclePolicy::transaction_read_only()
    );
    assert_eq!(
        TransactionAction::GetAllHolons.policy(),
        CommandLifecyclePolicy::transaction_read_only()
    );
    assert_eq!(
        TransactionAction::NewHolon { key: None }.policy(),
        CommandLifecyclePolicy::mutating()
    );
    assert_eq!(
        TransactionAction::DeleteHolon { local_id: LocalId(vec![]) }.policy(),
        CommandLifecyclePolicy::mutating()
    );
}

#[test]
fn holon_action_policies() {
    assert_eq!(
        HolonAction::Read(ReadableHolonAction::Key).policy(),
        CommandLifecyclePolicy::holon_read_only()
    );
    assert_eq!(
        HolonAction::Read(ReadableHolonAction::CloneHolon).policy(),
        CommandLifecyclePolicy::mutating(),
        "CloneHolon creates a transient — mutating despite being a ReadableHolonAction"
    );
    assert_eq!(
        HolonAction::Write(WritableHolonAction::WithPropertyValue {
            name: PropertyName(MapString::from("x")),
            value: BaseValue::StringValue(MapString::from("v")),
        })
        .policy(),
        CommandLifecyclePolicy::mutating()
    );
}
