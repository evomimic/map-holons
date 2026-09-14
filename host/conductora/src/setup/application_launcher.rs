//! Rust-owned application-startup session state.
//!
//! This module deliberately contains no Canvas, Visualizer, or Dancer policy.
//! It records the MAP-bound application context established by Conductora so a
//! frontend can observe readiness without using the retired loader ingress.

use std::sync::RwLock;

use core_types::HolonId;
use serde::Serialize;

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
    Ready,
    Failed,
}

/// Runtime-only application context. It is never persisted as a holon and
/// never retains a transaction.
#[derive(Debug, Clone)]
struct ApplicationSession {
    experience: ApplicationExperience,
    phase: ApplicationSessionPhase,
    active_holon_space: Option<HolonId>,
    failure: Option<String>,
}

/// Serializable projection exposed to the thin TypeScript readiness client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApplicationSessionSnapshot {
    pub experience: ApplicationExperience,
    pub phase: ApplicationSessionPhase,
    pub active_holon_space: Option<String>,
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

    pub fn mark_ready(&self, active_holon_space: HolonId) -> Result<(), String> {
        let mut session = self.write()?;
        session.phase = ApplicationSessionPhase::Ready;
        session.active_holon_space = Some(active_holon_space);
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
            active_holon_space: session.active_holon_space.as_ref().map(ToString::to_string),
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
    use core_types::LocalId;

    #[test]
    fn session_is_ready_only_after_a_local_space_is_available() {
        let state = ApplicationSessionState::new(ApplicationExperience::Canvas);
        assert_eq!(state.snapshot().unwrap().phase, ApplicationSessionPhase::InitializingHost);

        state.mark_opening_space().unwrap();
        state.mark_bootstrapping_core().unwrap();
        state.mark_ready(HolonId::Local(LocalId(vec![7]))).unwrap();

        let snapshot = state.snapshot().unwrap();
        assert_eq!(snapshot.phase, ApplicationSessionPhase::Ready);
        assert_eq!(snapshot.experience, ApplicationExperience::Canvas);
        assert!(snapshot.active_holon_space.is_some());
        assert_eq!(snapshot.failure, None);
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
