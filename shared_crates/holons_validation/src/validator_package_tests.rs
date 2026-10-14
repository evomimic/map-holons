#[allow(dead_code)]
#[path = "test_support.rs"]
mod fixture;
use super::*;
use crate::{commit_assessment, ValidationCollector};
use holons_core::WritableHolon;

#[test]
fn bounded_package_preserves_findings_and_does_not_rediscover_schema() -> Result<(), HolonError> {
    for case in [
        "valid",
        "missing-required",
        "wrong-kind",
        "extra-property",
        "extra-relationship",
        "unsupported-constraint",
        "malformed-declaration",
    ] {
        let mut graph = fixture::readiness_fixture()?;
        graph
            .node("VisualizerUsage.HolonType")?
            .with_property_value("TypeName", "VisualizerUsage")?
            .with_property_value("DefinesInstanceTypeKind", false)?;
        for (edge, target) in [
            (R::Extends, "HolonType.TypeDescriptor"),
            (R::InstanceProperties, "Title.PropertyType"),
            (R::InstanceProperties, "Key.PropertyType"),
        ] {
            graph.link("VisualizerUsage.HolonType", edge, target)?;
        }
        if case == "unsupported-constraint" {
            graph.node("UnknownConstraint")?;
            graph.link("UnknownConstraint", R::DescribedBy, "ConstraintType.HolonType")?;
            graph.link("VisualizerUsage.HolonType", R::Constraints, "UnknownConstraint")?;
        }
        if case == "malformed-declaration" {
            graph.node("DescribedBy.Relationship")?.remove_related_holons(
                R::TargetType,
                vec![graph.nodes["HolonType.TypeDescriptor"].clone()],
            )?;
        }
        graph.node("OwningSchema")?;
        graph.link("OwningSchema", R::DependsOn, "OwningSchema")?;
        graph.link("OwningSchema", R::DescribedBy, "Schema.HolonType")?;
        graph.link("VisualizerUsage.HolonType", R::ComponentOf, "OwningSchema")?;
        graph.link(
            "VisualizerUsage.HolonType",
            R::DescribedBy,
            "MetaHolonType.MetaTypeDescriptor",
        )?;
        let mut results = Vec::new();
        for bounded in [false, true] {
            let snapshot = graph.saved_snapshot()?;
            let graph = &snapshot;
            let subject = graph.context.mutation().new_holon(Some(case.into()))?;
            let mut staged = graph.context.mutation().stage_new_holon(subject)?;
            staged.with_descriptor(graph.nodes["VisualizerUsage.HolonType"].clone())?;
            if case != "missing-required" {
                staged.with_property_value("Title", "present")?;
            }
            if case == "wrong-kind" {
                staged.with_property_value("Title", 42_i64)?;
            }
            if case == "extra-property" {
                staged.with_property_value("Unexpected", true)?;
            }
            if case == "extra-relationship" {
                staged.add_related_holons_ungoverned(
                    "Unexpected",
                    vec![graph.nodes["Contract"].clone()],
                )?;
            }
            let candidates = [staged];
            let reader = ProspectiveDescriptorReader::new(&graph.context, &candidates)?;
            let roots =
                commit_assessment::ReadinessContext::resolve(&graph.context, &reader).unwrap();
            let workset = SchemaWorkset { schemas: Vec::new(), ownership: HashMap::new() };
            let governors = [graph.nodes["VisualizerUsage.HolonType"].clone()];
            let subjects = [HolonReference::from(&candidates[0])];
            let mut assessment = commit_assessment::construct_assessment(
                &graph.context,
                &candidates,
                &reader,
                &roots,
                ValidationCollector::default(),
            )?;
            if !bounded {
                let legacy = DescriptorPackages::construct_descriptor_with_observer(
                    &graph.context,
                    &reader,
                    &governors,
                    &subjects,
                    &workset,
                    &mut |_| {},
                )?;
                assessment = assessment.with_test_packages(legacy);
            }
            let reads = graph.backend_relationship_reads();
            let (report, observations) = assessment.assess(|_| {})?;
            if bounded {
                assert_eq!(
                    graph.backend_relationship_reads(),
                    reads,
                    "no post-construction schema reads for {case}"
                );
            }
            if bounded {
                assert_eq!(
                    graph.backend_relationship_reads_for("OwningSchema")?,
                    0,
                    "ownership must not expand validator scope"
                );
            } else {
                assert!(
                    graph.backend_relationship_reads_for("OwningSchema")? > 0,
                    "control demonstrates legacy ownership expansion"
                );
            }
            let report = report.into_report();
            let normalized =
                format!("{report:?}").replace(&candidates[0].reference_id_string(), "SUBJECT");
            results.push((normalized, observations, report.is_accepted()));
        }
        assert_eq!(results[0], results[1], "complete findings and coverage for {case}");
        assert_eq!(results[1].2, case == "valid", "expected acceptance for {case}");
    }
    Ok(())
}
