//! Packaged Welcome admission and the bounded local constitution workflow.
use crate::{
    setup::application_launcher::{ApplicationSessionPhase, ApplicationSessionState},
    welcome_lifecycle::{self, AdmissionBlocker, WelcomeLifecycle},
};
use map_commands_runtime::personal_constitution::{ConstitutionTarget, SovereigntyObservation};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const WINDOW_LABEL: &str = "welcome";
pub const PAGE: &str = "welcome.html";

/// The supported packaged path cannot opt back into development privileges through environment.
pub fn validate_packaged_configuration() -> anyhow::Result<()> {
    if !cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        anyhow::bail!("Packaged Welcome currently supports macOS on Apple Silicon only");
    }
    anyhow::ensure!(
        !tauri::is_dev(),
        "Packaged Welcome requires the custom-protocol build feature"
    );
    if cfg!(feature = "local-constitution") {
        anyhow::ensure!(cfg!(debug_assertions), "Local constitution requires a debug build");
        anyhow::ensure!(
            !cfg!(feature = "protected-custody"),
            "Local constitution and signed custody profiles must not be combined"
        );
    }
    for name in ["MAP_START_MODE", "STORAGE_CONFIG_PATH", "MAP_APPLICATION_EXPERIENCE"] {
        if std::env::var_os(name).is_some() {
            anyhow::bail!(
                "{name} is a development override and is not supported by packaged Welcome"
            );
        }
    }
    Ok(())
}

fn approved_page(label: &str, url: &tauri::Url) -> bool {
    label == WINDOW_LABEL
        && url.scheme() == "tauri"
        && url.host_str() == Some("localhost")
        && url.path() == "/welcome.html"
        && url.query().is_none()
        && url.fragment().is_none()
}

pub(crate) fn admit(window: &WebviewWindow) -> Result<(), String> {
    let url = window.url().map_err(|e| e.to_string())?;
    if crate::env::development_enabled() || !approved_page(window.label(), &url) {
        return Err("Request did not originate from packaged Welcome".into());
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WelcomePhase {
    Initializing,
    Provisioning,
    CheckingSpace,
    FoundationPresent,
    AssociationPresent,
    Blocked,
}

/// Safe presentation projection. No target IDs, credentials or execution permit cross IPC.
#[derive(Debug, Serialize)]
pub struct WelcomeStatus {
    pub phase: WelcomePhase,
    pub stage: ApplicationSessionPhase,
    pub constitution_available: bool,
    pub navigator_available: bool,
    pub message: &'static str,
    pub(crate) blockers: Vec<AdmissionBlocker>,
}

fn status_for(
    phase: ApplicationSessionPhase,
    observation: Option<Result<ConstitutionTarget, ()>>,
) -> WelcomeStatus {
    let stage = phase;
    let (phase, message, blockers) = if phase == ApplicationSessionPhase::Failed {
        (WelcomePhase::Blocked,
         "MAP could not finish opening this installation. Personal setup is unavailable; existing state has been retained.",
         vec![AdmissionBlocker::LifecycleEvidenceUnavailable])
    } else {
        match observation {
            Some(Ok(target)) => {
                let blockers = welcome_lifecycle::blockers(&target);
                match target.sovereignty {
                    SovereigntyObservation::NoAssociationObserved => (WelcomePhase::FoundationPresent,
                        "Your space and its foundation are present. Personal space setup is not enabled yet; acquisition eligibility and security protections still require verification.", blockers),
                    SovereigntyObservation::AssociationPresent => (WelcomePhase::AssociationPresent,
                        "A sovereign association is recorded for this space. MAP will not repeat constitution. Authorized reopening is not available in this build.", blockers),
                    SovereigntyObservation::Ambiguous => (WelcomePhase::Blocked,
                        "MAP could not resolve this space's sovereign association. Constitution and reopening are unavailable; existing state has been retained.", blockers),
                }
            }
            Some(Err(())) => (WelcomePhase::Blocked,
                "MAP could not verify this space and its required foundation. Personal setup is unavailable; existing state has been retained.",
                vec![AdmissionBlocker::LifecycleEvidenceUnavailable]),
            None => match phase {
                ApplicationSessionPhase::PreparingCoreSchema | ApplicationSessionPhase::LoadingCoreSchema | ApplicationSessionPhase::LoadingSpaceDefinitions =>
                    (WelcomePhase::Provisioning, "MAP is preparing your space and its foundation.", vec![AdmissionBlocker::StartupIncomplete]),
                ApplicationSessionPhase::OpeningSpace | ApplicationSessionPhase::VerifyingCoreSchema | ApplicationSessionPhase::ActivatingBasePackages | ApplicationSessionPhase::VerifyingSpaceDefinitions =>
                    (WelcomePhase::CheckingSpace, "MAP is checking your existing space and its foundation.", vec![AdmissionBlocker::StartupIncomplete]),
                _ => (WelcomePhase::Initializing, "MAP is starting its trusted local runtime.", vec![AdmissionBlocker::StartupIncomplete]),
            },
        }
    };
    WelcomeStatus {
        phase,
        stage,
        constitution_available: false,
        navigator_available: false,
        message,
        blockers,
    }
}

/// Local engineering admission is a compiled profile, never a frontend flag.
fn apply_local_workflow(status: &mut WelcomeStatus, available: bool) {
    status.navigator_available = false;
    if !cfg!(all(feature = "local-constitution", debug_assertions)) || tauri::is_dev() {
        return;
    }
    if !available
        && matches!(
            status.phase,
            WelcomePhase::FoundationPresent | WelcomePhase::AssociationPresent
        )
    {
        status.phase = WelcomePhase::Blocked;
        status.message = "A constitution transaction is in progress or retained for remediation. MAP will not create another Agent.";
        status.blockers = vec![AdmissionBlocker::ConstitutionTransactionPending];
    } else if status.phase == WelcomePhase::FoundationPresent {
        status.constitution_available = true;
        status.message = "Your space is ready. Enter your name to create your personal space.";
        status.blockers.clear();
    } else if status.phase == WelcomePhase::AssociationPresent {
        status.navigator_available = true;
        status.message = "Your existing personal space is open. MAP has retained your Agent and Space without repeating constitution.";
    }
}

#[tauri::command]
pub async fn welcome_status(
    window: WebviewWindow,
    application: State<'_, ApplicationSessionState>,
    lifecycle: State<'_, WelcomeLifecycle>,
    constitution: State<'_, map_commands_runtime::personal_constitution::PersonalConstitution>,
) -> Result<WelcomeStatus, String> {
    admit(&window)?;
    let phase = application.snapshot()?.phase;
    // Once startup has established the runtime, each status request refreshes
    // saved evidence. Polling must not replay a pre-Commit observation forever.
    if phase != ApplicationSessionPhase::Failed
        && lifecycle.observation()?.is_some()
        && constitution.available()
    {
        refresh_lifecycle(window.app_handle()).await.map_err(|e| e.to_string())?;
    }
    let mut status = status_for(phase, lifecycle.observation()?);
    apply_local_workflow(&mut status, constitution.available());
    Ok(status)
}

/// Refresh display evidence through the configured native runtime. Only startup
/// and the already-admitted constitution request can call this; it is not an IPC surface.
pub(crate) async fn refresh_lifecycle(handle: &AppHandle) -> anyhow::Result<()> {
    let runtime = handle
        .state::<crate::runtime::RuntimeState>()
        .read()
        .map_err(|e| anyhow::anyhow!(e.to_string()))?
        .clone()
        .ok_or_else(|| anyhow::anyhow!("MAP runtime is unavailable"))?;
    let observation = map_commands_runtime::personal_constitution::inspect_target(&runtime).await;
    if let Err(error) = &observation {
        tracing::warn!("Welcome lifecycle evidence unavailable: {error}");
    }
    handle
        .state::<WelcomeLifecycle>()
        .record(observation.map_err(|_| ()))
        .map_err(anyhow::Error::msg)
}

/// IPC projection of an ordinary Commit outcome.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConstitutionOutcome {
    Complete,
    Rejected,
    Incomplete,
}

impl From<map_commands_runtime::personal_constitution::ConstitutionOutcome>
    for ConstitutionOutcome
{
    fn from(outcome: map_commands_runtime::personal_constitution::ConstitutionOutcome) -> Self {
        use map_commands_runtime::personal_constitution::ConstitutionOutcome as Domain;
        match outcome {
            Domain::Complete => Self::Complete,
            Domain::Rejected => Self::Rejected,
            Domain::Incomplete => Self::Incomplete,
        }
    }
}

/// Untrusted naming data only. Resource, author and principal selection belongs
/// to the backend; these fields never carry an acquisition permit or a key.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersonalConstitutionInput {
    pub first_name: Option<String>,
    pub middle_name: Option<String>,
    pub last_name: String,
}

#[tauri::command]
pub async fn constitute_personal_space(
    window: WebviewWindow,
    input: PersonalConstitutionInput,
) -> Result<ConstitutionOutcome, String> {
    admit(&window)?;
    if !cfg!(all(feature = "local-constitution", debug_assertions)) {
        return Err("Personal constitution is not enabled in this build".into());
    }
    let handle = window.app_handle();
    let phase = handle.state::<ApplicationSessionState>().snapshot()?.phase;
    let observation = handle.state::<WelcomeLifecycle>().observation()?;
    if phase == ApplicationSessionPhase::Failed || observation.is_none() {
        return Err(status_for(phase, observation).message.into());
    }
    let runtime = handle
        .state::<crate::runtime::RuntimeState>()
        .read()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or("MAP runtime is unavailable")?;
    // Admission comes from the exact packaged request path. Fresh domain checks
    // and serialization happen in the backend, not in presentation state.
    let outcome = handle
        .state::<map_commands_runtime::personal_constitution::PersonalConstitution>()
        .submit(
            &runtime,
            map_commands_runtime::personal_constitution::PersonalName {
                last_name: input.last_name.into(),
                first_name: input.first_name.map(Into::into),
                middle_name: input.middle_name.map(Into::into),
            },
        )
        .await
        .map_err(|e| e.to_string())?;
    refresh_lifecycle(handle).await.map_err(|e| e.to_string())?;
    Ok(outcome.into())
}

pub fn create_window(handle: &tauri::AppHandle) -> anyhow::Result<()> {
    WebviewWindowBuilder::new(handle, WINDOW_LABEL, WebviewUrl::App(PAGE.into()))
        .title("Welcome to MAP")
        .inner_size(720.0, 760.0)
        .on_navigation(|url| approved_page(WINDOW_LABEL, url))
        .build()?;
    Ok(())
}

/// No conductor credentials or runtime references are exposed by this boundary.
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn constitution_input_accepts_only_names_without_rewriting_them() {
        let names: PersonalConstitutionInput = serde_json::from_value(serde_json::json!({
            "lastName": "van den Berg", "firstName": " Élodie ", "middleName": null
        }))
        .unwrap();
        assert_eq!(names.last_name, "van den Berg");
        assert_eq!(names.first_name.as_deref(), Some(" Élodie "));
        assert_eq!(names.middle_name, None);
        let minimal: PersonalConstitutionInput =
            serde_json::from_value(serde_json::json!({"lastName": "李"})).unwrap();
        assert_eq!(minimal.last_name, "李");
        assert_eq!(minimal.first_name, None);
        assert_eq!(minimal.middle_name, None);
        for invalid in [
            serde_json::json!({}),
            serde_json::json!({"lastName":null}),
            serde_json::json!({"lastName":12}),
        ] {
            assert!(serde_json::from_value::<PersonalConstitutionInput>(invalid).is_err());
        }
    }

    #[test]
    fn constitution_input_rejects_authority_fields_and_unknown_properties() {
        for field in [
            "spaceId",
            "agentId",
            "agentPubKey",
            "bootstrap",
            "credential",
            "permit",
            "key",
            "properties",
        ] {
            let mut value = serde_json::json!({"lastName":"Example"});
            value[field] = serde_json::json!("untrusted assertion");
            assert!(
                serde_json::from_value::<PersonalConstitutionInput>(value).is_err(),
                "accepted {field}"
            );
        }
    }

    #[test]
    fn welcome_permissions_exclude_conductor_and_generic_map_commands() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/welcome.json")).unwrap();
        assert_eq!(capability["webviews"], serde_json::json!(["welcome"]));
        assert!(capability.get("windows").is_none());
        assert!(capability.get("remote").is_none());
        assert_eq!(
            capability["permissions"],
            serde_json::json!([
                "allow-welcome-status",
                "allow-constitute-personal-space",
                "allow-open-space-navigator"
            ])
        );
        let page = include_str!("../../ui/public/welcome.html");
        assert!(page.contains("script-src 'self';"));
        assert!(page.contains("frame-src 'none';"));
        assert!(!page.contains("blob:"));
    }

    #[test]
    fn welcome_distinguishes_startup_evidence_and_authorization() {
        use core_types::{HolonId, LocalId};
        let target = |sovereignty| ConstitutionTarget {
            space: HolonId::Local(LocalId(vec![9; 39])),
            sovereignty,
        };
        let startup = status_for(ApplicationSessionPhase::LoadingCoreSchema, None);
        assert_eq!(startup.phase, WelcomePhase::Provisioning);
        assert_eq!(startup.stage, ApplicationSessionPhase::LoadingCoreSchema);
        let provisioned = status_for(
            ApplicationSessionPhase::ActivatingBasePackages,
            Some(Ok(target(SovereigntyObservation::NoAssociationObserved))),
        );
        assert_eq!(provisioned.phase, WelcomePhase::FoundationPresent);
        assert!(provisioned.blockers.contains(&AdmissionBlocker::CredentialCustodyUnverified));
        let acquired = status_for(
            ApplicationSessionPhase::ActivatingBasePackages,
            Some(Ok(target(SovereigntyObservation::AssociationPresent))),
        );
        assert_eq!(acquired.phase, WelcomePhase::AssociationPresent);
        assert!(acquired.blockers.contains(&AdmissionBlocker::AssociationAlreadyEstablished));
        let ambiguous = status_for(
            ApplicationSessionPhase::ActivatingBasePackages,
            Some(Ok(target(SovereigntyObservation::Ambiguous))),
        );
        assert_eq!(ambiguous.phase, WelcomePhase::Blocked);
        let failed = status_for(
            ApplicationSessionPhase::Failed,
            Some(Ok(target(SovereigntyObservation::NoAssociationObserved))),
        );
        assert_eq!(failed.phase, WelcomePhase::Blocked);
        let unavailable =
            status_for(ApplicationSessionPhase::ActivatingBasePackages, Some(Err(())));
        assert_eq!(unavailable.phase, WelcomePhase::Blocked);
        for status in [startup, provisioned, acquired, ambiguous, failed, unavailable] {
            assert!(!status.constitution_available);
            assert!(!status.blockers.is_empty());
            let wire = serde_json::to_value(status).unwrap();
            assert!(wire.get("space").is_none());
            assert!(wire.get("credential").is_none());
        }
    }

    #[test]
    fn local_workflow_requires_the_compiled_profile_and_an_available_attempt() {
        use core_types::{HolonId, LocalId};
        let mut status = status_for(
            ApplicationSessionPhase::ActivatingBasePackages,
            Some(Ok(ConstitutionTarget {
                space: HolonId::Local(LocalId(vec![9; 39])),
                sovereignty: SovereigntyObservation::NoAssociationObserved,
            })),
        );
        apply_local_workflow(&mut status, true);
        assert_eq!(
            status.constitution_available,
            cfg!(all(feature = "local-constitution", debug_assertions)) && !tauri::is_dev()
        );
        status.constitution_available = false;
        apply_local_workflow(&mut status, false);
        assert!(!status.constitution_available);
        if cfg!(all(feature = "local-constitution", debug_assertions)) && !tauri::is_dev() {
            assert_eq!(status.phase, WelcomePhase::Blocked);
            assert_eq!(status.blockers, vec![AdmissionBlocker::ConstitutionTransactionPending]);
            status.phase = WelcomePhase::AssociationPresent;
            apply_local_workflow(&mut status, false);
            assert_eq!(
                status.phase,
                WelcomePhase::Blocked,
                "retained work is not a completed Open"
            );
        }
    }

    #[test]
    fn only_exact_packaged_welcome_page_is_admitted() {
        let trusted = tauri::Url::parse("tauri://localhost/welcome.html").unwrap();
        assert!(approved_page("welcome", &trusted));
        assert!(!approved_page("main", &trusted));
        assert!(!approved_page("welcome-untrusted-content", &trusted));
        for url in [
            "http://localhost:4200/welcome.html",
            "https://example.org/welcome.html",
            "tauri://localhost/index.html",
            "tauri://localhost/welcome.html?bootstrap=true",
            "blob:tauri://localhost/code",
        ] {
            assert!(!approved_page("welcome", &tauri::Url::parse(url).unwrap()), "{url}");
        }
    }
}
