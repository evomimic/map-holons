//! Existing Canvas/home-Dancer selection over an already initialized runtime.
use crate::Runtime;
use serde::Serialize;

/// Observable stage of one window-bound MAP application session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApplicationSessionPhase {
    InitializingHost,
    ActivatingHolochainApp,
    OpeningSpace,
    PreparingCoreSchema,
    LoadingCoreSchema,
    VerifyingCoreSchema,
    ActivatingBasePackages,
    LoadingSpaceDefinitions,
    VerifyingSpaceDefinitions,
    RealizingCanvas,
    SelectingHomeDancer,
    RealizingHomeDancer,
    Ready,
    Failed,
}

use base_types::MapString;
use core_types::{HolonError, HolonId};
use holons_core::{
    agent_spaces::{current_personal_space, sovereign_agent, SovereignAgent},
    HolonReference, ReadableHolon,
};

#[derive(Clone, Copy)]
pub enum LaunchRequirement {
    ProvisionedSpace,
    AcquiredSpace,
}
#[derive(Clone, Copy)]
pub enum LaunchStage {
    Canvas,
    HomeDancer,
    DancerRealization,
}

pub struct HomeDancerLaunch {
    pub dancer: HolonId,
    pub rooted_navigation_visualizer: HolonId,
    pub root_node_visualizer: HolonId,
}
pub struct ApplicationLaunch {
    pub space: HolonId,
    pub agent: Option<HolonId>,
    pub theme_key: String,
    pub canvas_key: String,
    pub canvas_visualizer_key: String,
    pub home: Option<HomeDancerLaunch>,
}

/// Reuse the runtime and adopted Space; never provision, import or constitute.
/// The caller supplies authorization. Observing a sovereign Agent grants none.
pub async fn select_application_launch(
    runtime: &Runtime,
    requirement: LaunchRequirement,
    stage: impl Fn(LaunchStage) -> Result<(), HolonError>,
) -> Result<ApplicationLaunch, HolonError> {
    let tx = runtime.session().begin_transaction().await?;
    let selected = (|| {
        let context = runtime.session().get_transaction(&tx)?;
        let space = current_personal_space(&context)?;
        let person = match requirement {
            LaunchRequirement::ProvisionedSpace => None,
            LaunchRequirement::AcquiredSpace => match sovereign_agent(&space)? {
                SovereignAgent::Agent(agent) => Some(agent),
                _ => {
                    return Err(HolonError::InvalidState(
                        "Navigator requires one saved sovereign Agent association".into(),
                    ))
                }
            },
        };
        stage(LaunchStage::Canvas)?;
        let selection = dahn_selection::select_bootstrap_canvas(&context)?;
        let theme_key = selection
            .theme
            .key()?
            .ok_or_else(|| HolonError::InvalidState("Selected Theme has no key".into()))?
            .to_string();
        let canvas_key = selection
            .canvas_visualizer
            .canvas
            .key()?
            .ok_or_else(|| HolonError::InvalidState("Selected Canvas has no key".into()))?
            .to_string();
        let canvas_visualizer_key = selection
            .canvas_visualizer
            .visualizer
            .key()?
            .ok_or_else(|| {
                HolonError::InvalidState("Selected Canvas Visualizer has no key".into())
            })?
            .to_string();
        stage(LaunchStage::HomeDancer)?;
        let meta_design_system = HolonReference::Smart(
            context.lookup().get_saved_holon_by_key(&MapString::from("Demo1.MetaDesignSystem"))?,
        );
        let home = dahn_selection::select_home_dancer(
            &context,
            dahn_selection::HomeDancerSelectionContext {
                active_holon_space: space.clone(),
                selected_theme: selection.theme,
                selected_meta_design_system: meta_design_system,
                runtime: dahn_selection::HomeDancerRuntime::Local,
                person: person.clone(),
            },
        )?
        .map(|home| {
            Ok::<_, HolonError>(HomeDancerLaunch {
                dancer: home.dancer.holon_id()?,
                rooted_navigation_visualizer: home.rooted_navigation_visualizer.holon_id()?,
                root_node_visualizer: home.root_node_visualizer.holon_id()?,
            })
        })
        .transpose()?;
        if matches!(requirement, LaunchRequirement::AcquiredSpace) && home.is_none() {
            return Err(HolonError::InvalidState(
                "The acquired Space does not afford a compatible home Dancer".into(),
            ));
        }
        stage(LaunchStage::DancerRealization)?;
        Ok(ApplicationLaunch {
            space: space.holon_id()?,
            agent: person.map(|p| p.holon_id()).transpose()?,
            theme_key,
            canvas_key,
            canvas_visualizer_key,
            home,
        })
    })();
    let archived = runtime.session().archive_transaction(&tx);
    let selected = selected?;
    archived?;
    tracing::info!(space = ?selected.space, agent = ?selected.agent, "[LAUNCH] Selected Canvas and home Dancer using the existing Space and Agent");
    Ok(selected)
}
