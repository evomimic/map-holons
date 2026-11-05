use core_types::HolonError;
use holons_core::{
    dances::{DanceRequest, DanceType, RequestBody},
    HolonReference,
};

/// Internal service transport for the Visualizer-afforded read-only resolution capability.
pub fn build_resolve_visualizer_usage_dance_request(
    visualizer: HolonReference,
    subject_type: HolonReference,
    slot: HolonReference,
) -> Result<DanceRequest, HolonError> {
    Ok(DanceRequest::new(
        "resolve_visualizer_usage".into(),
        DanceType::Standalone,
        RequestBody::TargetHolons(
            core_types::RelationshipName("UsageResolutionInputs".into()),
            vec![visualizer, subject_type, slot],
        ),
    ))
}
