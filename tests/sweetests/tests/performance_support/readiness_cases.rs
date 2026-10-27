//! Bounded semantic controls run after timed samples, in the same isolated runtime.
use holons_prelude::prelude::*;
use map_commands_contract::{
    MapCommand, MapResult, SpaceCommand, TransactionAction, TransactionCommand,
};
use map_commands_runtime::{ExecutionPolicy, Runtime};
use serde_json::json;

pub async fn run(runtime: &Runtime, output: &str, mode: &str) {
    let mut results = Vec::new();
    for case in
        ["valid", "extra-property", "wrong-key-kind", "undeclared-relationship", "missing-key"]
    {
        let MapResult::TransactionCreated { tx_id } = runtime
            .execute_command(
                MapCommand::Space(SpaceCommand::BeginTransaction),
                ExecutionPolicy::default(),
            )
            .await
            .unwrap()
        else {
            panic!("BeginTransaction must return a transaction")
        };
        let context = runtime.session().get_transaction(&tx_id).unwrap();
        let visualizer: HolonReference = context
            .lookup()
            .get_saved_holon_by_key(&MapString::from(
                "DefaultPropertyMapVisualizer.PropertyMapVisualizer",
            ))
            .unwrap()
            .into();
        let mut staged = if mode == "visualizer-slot" {
            super::creation_samples::stage_visualizer_slot(&context, &format!("readiness-{case}"))
        } else {
            assert_eq!(mode, "create");
            let descriptor: HolonReference = context
                .lookup()
                .get_saved_holon_by_key(&MapString::from("VisualizerUsage.HolonType"))
                .unwrap()
                .into();
            let mut holon =
                context.mutation().new_holon(Some(format!("readiness-{case}").into())).unwrap();
            holon.add_related_holons("DescribedBy", vec![descriptor]).unwrap();
            holon.add_related_holons("UsesVisualizer", vec![visualizer.clone()]).unwrap();
            context.mutation().stage_new_holon(holon).unwrap()
        };
        match case {
            "extra-property" => {
                staged.with_property_value("Unexpected", true).unwrap();
            }
            "wrong-key-kind" => {
                staged.with_property_value("Key", 42_i64).unwrap();
            }
            "undeclared-relationship" => {
                staged
                    .add_related_holons_ungoverned("UnexpectedRelationship", vec![visualizer])
                    .unwrap();
            }
            "missing-key" => {
                staged.remove_property_value("Key").unwrap();
            }
            _ => {}
        }
        let outcome = runtime
            .execute_command(
                MapCommand::Transaction(TransactionCommand {
                    context: context.clone(),
                    action: TransactionAction::Commit,
                }),
                ExecutionPolicy::default(),
            )
            .await;
        if case == "missing-key" {
            let error = outcome.expect_err("current persistence requires a Key");
            assert!(error.to_string().contains("Committed holon has no key"), "{error}");
            results.push(json!({"case":case,"outcome":"operational-error","error":"Committed holon has no key"}));
            std::fs::write(output, serde_json::to_vec_pretty(&results).unwrap()).unwrap();
            runtime.session().archive_transaction(&tx_id).unwrap();
            continue;
        }
        let MapResult::Reference(response) = outcome.unwrap() else {
            panic!("Commit response required")
        };
        let status = response.property_value("CommitRequestStatus").unwrap();
        let count = response.property_value("ValidationViolationCount").unwrap();
        let saved =
            response.related_holons("SavedHolons").unwrap().read().unwrap().get_members().len();
        let rejected =
            matches!(case, "extra-property" | "wrong-key-kind" | "undeclared-relationship");
        assert_eq!(
            status,
            Some(if rejected { "Rejected" } else { "Complete" }.to_base_value()),
            "case {case}"
        );
        assert_eq!(saved, if rejected { 0 } else { 1 }, "case {case}");
        let findings = staged.validation_findings().unwrap();
        let signatures: Vec<_> = findings
            .iter()
            .map(|finding| {
                json!({
                    "kind":finding.kind, "rule_key":finding.rule_key,
                })
            })
            .collect();
        results.push(json!({"case":case,"status":status,"violation_count":count,
            "saved":saved,"findings":signatures}));
        std::fs::write(output, serde_json::to_vec_pretty(&results).unwrap()).unwrap();
        if case == "extra-property" {
            staged.remove_property_value("Unexpected").unwrap();
            let MapResult::Reference(retry) = runtime
                .execute_command(
                    MapCommand::Transaction(TransactionCommand {
                        context: context.clone(),
                        action: TransactionAction::Commit,
                    }),
                    ExecutionPolicy::default(),
                )
                .await
                .unwrap()
            else {
                panic!("Commit response required")
            };
            assert_eq!(
                retry.property_value("CommitRequestStatus").unwrap(),
                Some("Complete".to_base_value())
            );
            assert_eq!(
                retry.property_value("ValidationViolationCount").unwrap(),
                Some(0_i64.to_base_value())
            );
            results.push(json!({"case":"corrected-retry","status":"Complete","violation_count":0}));
        }
        runtime.session().archive_transaction(&tx_id).unwrap();
    }
    std::fs::write(output, serde_json::to_vec_pretty(&results).unwrap()).unwrap();
}
