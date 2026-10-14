#[allow(dead_code)]
#[path = "test_support.rs"]
mod fixture;
use super::*;
use holons_core::WritableHolon;

#[test]
fn instance_reports_match_without_saved_key_resolution() -> Result<(), HolonError> {
    for case in ["valid", "missing-required", "wrong-kind", "extra-property", "extra-relationship"]
    {
        let mut graph = fixture::readiness_fixture()?;
        graph
            .node("VisualizerUsage.HolonType")?
            .with_property_value("TypeName", "VisualizerUsage")?
            .with_property_value("DefinesInstanceTypeKind", false)?;
        for (edge, target) in [
            (R::Extends, "HolonType.TypeDescriptor"),
            (R::InstanceProperties, "Title.PropertyType"),
            (R::InstanceProperties, "Key.PropertyType"),
            (R::DescribedBy, "MetaHolonType.MetaTypeDescriptor"),
        ] {
            graph.link("VisualizerUsage.HolonType", edge, target)?;
        }
        let graph = graph.saved_snapshot()?;
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
            crate::commit_assessment::ReadinessContext::resolve(&graph.context, &reader).unwrap();
        let (baseline, baseline_observations) = crate::commit_assessment::construct_assessment(
            &graph.context,
            &candidates,
            &reader,
            &roots,
            ValidationCollector::default(),
        )?
        .assess(|_| {})?;
        let keys_before = graph.backend_key_reads();
        assert!(keys_before > 0);
        let (trial, observations) =
            assess(&graph.context, &candidates, &reader, &mut |_| {})?.expect("eligible instance");
        assert_eq!(graph.backend_key_reads(), keys_before, "no saved key resolution for {case}");
        let baseline = baseline.into_report();
        let trial = trial.into_report();
        assert_eq!(baseline, trial, "complete findings for {case}");
        assert_eq!(baseline_observations, observations, "coverage for {case}");
        assert_eq!(trial.is_accepted(), case == "valid", "expected acceptance for {case}");
        if case == "valid" {
            let update = graph.replacement("VisualizerUsage.HolonType")?;
            let mixed = [candidates[0].clone(), update];
            let prospective = ProspectiveDescriptorReader::new(&graph.context, &mixed)?;
            assert!(
                assess(&graph.context, &mixed, &prospective, &mut |_| {})?.is_none(),
                "staged governing definitions retain the complete assessment path"
            );
        }
    }
    Ok(())
}
