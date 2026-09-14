//! Rust-owned application-startup session state.
//!
//! This module deliberately contains no Canvas, Visualizer, or Dancer policy.
//! It records the MAP-bound application context established by Conductora so a
//! frontend can observe readiness without using the retired loader ingress.

use std::sync::RwLock;

use holons_boundary::HolonReferenceWire;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CanvasLaunchSelection {
    pub theme_key: String,
    pub canvas_key: String,
    pub canvas_visualizer_key: String,
}

/// Rust-selected Dancer realization to be materialized by the thin UI client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HomeDancerLaunchSelection {
    pub dancer: HolonReferenceWire,
    pub node_visualizer: HolonReferenceWire,
}

/// The frontend experience selected by the MAP Application Launcher.
///
/// This is deliberately application configuration rather than a visualizer
/// selection fallback. Each experience is responsible for realizing the
/// session that Conductora has already established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApplicationExperience {
    Canvas,
    HolonsLoader,
}

impl ApplicationExperience {
    /// Resolves the explicitly configured experience, defaulting to Canvas.
    pub fn from_environment() -> Result<Self, String> {
        match std::env::var("MAP_APPLICATION_EXPERIENCE") {
            Ok(value) => Self::parse(&value),
            Err(std::env::VarError::NotPresent) => Ok(Self::Canvas),
            Err(error) => Err(format!("Could not read MAP_APPLICATION_EXPERIENCE: {error}")),
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "canvas" => Ok(Self::Canvas),
            "holons-loader" => Ok(Self::HolonsLoader),
            _ => Err(format!(
                "Unsupported MAP_APPLICATION_EXPERIENCE '{value}'. Expected 'canvas' or 'holons-loader'."
            )),
        }
    }
}

/// Observable stage of one window-bound MAP application session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApplicationSessionPhase {
    InitializingHost,
    OpeningSpace,
    BootstrappingCore,
    ActivatingBasePackages,
    RealizingCanvas,
    SelectingHomeDancer,
    RealizingHomeDancer,
    Ready,
    Failed,
}

/// Runtime-only application context. It is never persisted as a holon and
/// never retains a transaction.
#[derive(Debug, Clone)]
struct ApplicationSession {
    experience: ApplicationExperience,
    phase: ApplicationSessionPhase,
    active_holon_space: Option<HolonReferenceWire>,
    canvas_selection: Option<CanvasLaunchSelection>,
    home_dancer_selection: Option<HomeDancerLaunchSelection>,
    failure: Option<String>,
}

/// Serializable projection exposed to the thin TypeScript readiness client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApplicationSessionSnapshot {
    pub experience: ApplicationExperience,
    pub phase: ApplicationSessionPhase,
    pub active_holon_space: Option<HolonReferenceWire>,
    pub canvas_selection: Option<CanvasLaunchSelection>,
    pub home_dancer_selection: Option<HomeDancerLaunchSelection>,
    pub failure: Option<String>,
}

/// Tauri-managed state for the current application window.
#[derive(Debug)]
pub struct ApplicationSessionState {
    session: RwLock<ApplicationSession>,
}

impl Default for ApplicationSessionState {
    fn default() -> Self {
        Self::new(ApplicationExperience::Canvas)
    }
}

impl ApplicationSessionState {
    pub fn new(experience: ApplicationExperience) -> Self {
        Self {
            session: RwLock::new(ApplicationSession {
                experience,
                phase: ApplicationSessionPhase::InitializingHost,
                active_holon_space: None,
                canvas_selection: None,
                home_dancer_selection: None,
                failure: None,
            }),
        }
    }

    pub fn mark_opening_space(&self) -> Result<(), String> {
        self.set_phase(ApplicationSessionPhase::OpeningSpace)
    }

    pub fn mark_bootstrapping_core(&self) -> Result<(), String> {
        self.set_phase(ApplicationSessionPhase::BootstrappingCore)
    }

    pub fn mark_realizing_canvas(&self) -> Result<(), String> {
        self.set_phase(ApplicationSessionPhase::RealizingCanvas)
    }

    pub fn mark_activating_base_packages(&self) -> Result<(), String> {
        self.set_phase(ApplicationSessionPhase::ActivatingBasePackages)
    }

    pub fn mark_selecting_home_dancer(&self) -> Result<(), String> {
        self.set_phase(ApplicationSessionPhase::SelectingHomeDancer)
    }

    pub fn mark_realizing_home_dancer(&self) -> Result<(), String> {
        self.set_phase(ApplicationSessionPhase::RealizingHomeDancer)
    }

    pub fn mark_ready(
        &self,
        active_holon_space: HolonReferenceWire,
        canvas_selection: CanvasLaunchSelection,
        home_dancer_selection: Option<HomeDancerLaunchSelection>,
    ) -> Result<(), String> {
        let mut session = self.write()?;
        session.phase = ApplicationSessionPhase::Ready;
        session.active_holon_space = Some(active_holon_space);
        session.canvas_selection = Some(canvas_selection);
        session.home_dancer_selection = home_dancer_selection;
        session.failure = None;
        Ok(())
    }

    pub fn mark_failed(&self, failure: impl Into<String>) -> Result<(), String> {
        let mut session = self.write()?;
        session.phase = ApplicationSessionPhase::Failed;
        session.failure = Some(failure.into());
        Ok(())
    }

    pub fn snapshot(&self) -> Result<ApplicationSessionSnapshot, String> {
        let session = self
            .session
            .read()
            .map_err(|error| format!("ApplicationSession lock poisoned: {error}"))?;
        Ok(ApplicationSessionSnapshot {
            experience: session.experience,
            phase: session.phase,
            active_holon_space: session.active_holon_space.clone(),
            canvas_selection: session.canvas_selection.clone(),
            home_dancer_selection: session.home_dancer_selection.clone(),
            failure: session.failure.clone(),
        })
    }

    fn set_phase(&self, phase: ApplicationSessionPhase) -> Result<(), String> {
        let mut session = self.write()?;
        session.phase = phase;
        session.failure = None;
        Ok(())
    }

    fn write(&self) -> Result<std::sync::RwLockWriteGuard<'_, ApplicationSession>, String> {
        self.session.write().map_err(|error| format!("ApplicationSession lock poisoned: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_types::{HolonId, LocalId};
    use holons_boundary::SmartReferenceWire;
    use holons_core::core_shared_objects::transactions::TxId;

    fn space_reference() -> HolonReferenceWire {
        HolonReferenceWire::Smart(SmartReferenceWire::new(
            TxId::from_str("0").expect("fixture transaction id"),
            HolonId::Local(LocalId(vec![7])),
            None,
        ))
    }

    #[test]
    fn session_is_ready_only_after_a_local_space_is_available() {
        let state = ApplicationSessionState::new(ApplicationExperience::Canvas);
        assert_eq!(state.snapshot().unwrap().phase, ApplicationSessionPhase::InitializingHost);

        state.mark_opening_space().unwrap();
        state.mark_bootstrapping_core().unwrap();
        state
            .mark_ready(
                space_reference(),
                CanvasLaunchSelection {
                    theme_key: "MAP.BootstrapTheme".into(),
                    canvas_key: "MAP.BootstrapCanvas".into(),
                    canvas_visualizer_key: "MAP.BootstrapCanvasVisualizer".into(),
                },
                None,
            )
            .unwrap();

        let snapshot = state.snapshot().unwrap();
        assert_eq!(snapshot.phase, ApplicationSessionPhase::Ready);
        assert_eq!(snapshot.experience, ApplicationExperience::Canvas);
        assert_eq!(snapshot.active_holon_space, Some(space_reference()));
        assert_eq!(snapshot.canvas_selection.unwrap().canvas_key, "MAP.BootstrapCanvas");
        assert_eq!(snapshot.home_dancer_selection, None);
        assert_eq!(snapshot.failure, None);
    }

    #[test]
    fn session_projects_home_dancer_selection_after_its_observable_stages() {
        let state = ApplicationSessionState::new(ApplicationExperience::Canvas);
        state.mark_activating_base_packages().unwrap();
        assert_eq!(
            state.snapshot().unwrap().phase,
            ApplicationSessionPhase::ActivatingBasePackages
        );
        state.mark_selecting_home_dancer().unwrap();
        assert_eq!(state.snapshot().unwrap().phase, ApplicationSessionPhase::SelectingHomeDancer);
        state.mark_realizing_home_dancer().unwrap();

        let home_dancer_selection = HomeDancerLaunchSelection {
            dancer: space_reference(),
            node_visualizer: space_reference(),
        };
        state
            .mark_ready(
                space_reference(),
                CanvasLaunchSelection {
                    theme_key: "MAP.BootstrapTheme".into(),
                    canvas_key: "MAP.BootstrapCanvas".into(),
                    canvas_visualizer_key: "MAP.BootstrapCanvasVisualizer".into(),
                },
                Some(home_dancer_selection.clone()),
            )
            .unwrap();

        assert_eq!(state.snapshot().unwrap().home_dancer_selection, Some(home_dancer_selection));
    }

    #[test]
    fn failure_retains_its_diagnostic_without_inventing_a_ready_space() {
        let state = ApplicationSessionState::new(ApplicationExperience::HolonsLoader);
        state.mark_failed("Core bootstrap failed").unwrap();

        let snapshot = state.snapshot().unwrap();
        assert_eq!(snapshot.phase, ApplicationSessionPhase::Failed);
        assert_eq!(snapshot.experience, ApplicationExperience::HolonsLoader);
        assert_eq!(snapshot.active_holon_space, None);
        assert_eq!(snapshot.failure.as_deref(), Some("Core bootstrap failed"));
    }

    #[test]
    fn only_supported_application_experiences_are_accepted() {
        assert_eq!(ApplicationExperience::parse("canvas").unwrap(), ApplicationExperience::Canvas);
        assert_eq!(
            ApplicationExperience::parse("holons-loader").unwrap(),
            ApplicationExperience::HolonsLoader
        );
        assert!(ApplicationExperience::parse("loader").is_err());
    }
}
