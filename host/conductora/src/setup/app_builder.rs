use anyhow::Context;
use std::sync::RwLock;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    config::{providers::ProviderRuntimeSelection, storage_manager::StorageManager},
    map_commands as commands, runtime,
    setup::{
        application_launcher::{ApplicationExperience, ApplicationSessionState},
        core_schema_bootstrap::{ensure_core_schema_space, CoreSchemaBootstrapGate},
        plugin_manager::PluginManager,
        provider_registry::ProviderRegistry,
        receptor_config_registry::ReceptorConfigRegistry,
        setup_manager::SetupManager,
        space_imports::ensure_configured_space_imports,
    },
};

use client_shared_types::storage_receptor::ActiveStorageReceptor;
use holons_client::deprecated_receptor_factory::DeprecatedReceptorFactory;

pub struct AppBuilder;

const STARTUP_READY_EVENT: &str = "startup:ready";

impl AppBuilder {
    /// Build and configure the Tauri application
    pub fn build() -> tauri::Builder<tauri::Wry> {
        tracing::debug!("[APP BUILDER] Loading storage configuration.");
        let storage_manager = StorageManager::load_storage_config().unwrap_or_else(|e| {
            tracing::error!("[APP BUILDER] failed: {}", e);
            std::process::exit(1);
        });

        tracing::debug!("[APP BUILDER] Resolving runtime provider selection.");
        let runtime_selection = StorageManager::resolve_runtime_selection(&storage_manager)
            .unwrap_or_else(|e| {
                tracing::error!("[APP BUILDER] failed: {}", e);
                std::process::exit(1);
            });
        if runtime_selection.runtime_provider_keys.is_empty() {
            tracing::error!("[APP BUILDER] failed: at least one storage provider must be enabled.");
            std::process::exit(1);
        }
        for warning in &runtime_selection.warnings {
            tracing::warn!("[APP BUILDER] {}", warning);
        }
        tracing::debug!(
            "[APP BUILDER] configuration complete. runtime_providers={:?}, window_provider={:?}",
            runtime_selection.runtime_provider_keys,
            runtime_selection.window_provider_key
        );

        tracing::debug!("[APP BUILDER] Building provider registry.");
        let (runtime_provider_types, registry_warnings) =
            storage_manager.runtime_provider_types(&runtime_selection);
        for warning in registry_warnings {
            tracing::warn!("[APP BUILDER] {}", warning);
        }
        if runtime_provider_types.is_empty() {
            tracing::error!(
                "[APP BUILDER] failed: no runtime provider types resolved from runtime selection."
            );
            std::process::exit(1);
        }

        let registry = ProviderRegistry::with_provider_types(&runtime_provider_types);

        let application_experience =
            ApplicationExperience::from_environment().unwrap_or_else(|e| {
                tracing::error!("[APP BUILDER] failed: {}", e);
                std::process::exit(1);
            });
        tracing::info!(
            "[APP BUILDER] application experience selected: {:?}",
            application_experience
        );

        tracing::debug!("[APP BUILDER] Building base Tauri app.");
        let base = tauri::Builder::default()
            .manage(storage_manager.clone())
            .manage::<ProviderRuntimeSelection>(runtime_selection.clone())
            .manage::<runtime::RuntimeInitiatorState>(RwLock::new(None))
            .manage::<ActiveStorageReceptor>(RwLock::new(None))
            .manage(DeprecatedReceptorFactory::new())
            .manage(ReceptorConfigRegistry::new())
            .manage::<runtime::SessionReceptorState>(RwLock::new(None))
            //.manage::<runtime::HolochainReceptorState>(RwLock::new(None))
            .manage::<runtime::RuntimeState>(RwLock::new(None))
            .manage(CoreSchemaBootstrapGate::new())
            .manage(crate::config::environment::EnvironmentMismatch::default())
            .manage(crate::welcome_lifecycle::WelcomeLifecycle::default())
            .manage(map_commands_runtime::personal_constitution::PersonalConstitution::default())
            .manage(ApplicationSessionState::new(application_experience))
            .manage(crate::packaged_navigator::NavigatorLaunchLock::default());
        let base = if crate::env::development_enabled() {
            base.invoke_handler(tauri::generate_handler![
                application_session,
                crate::source_ingress::commands::select_loader_sources,
                crate::source_ingress::picker::source_picker_capabilities,
                commands::root_space,
                commands::serde_test,
                commands::map_request,
                commands::all_spaces,
                commands::is_service_ready,
                runtime::dispatch_map_command::dispatch_map_command,
            ])
        } else {
            base.invoke_handler(tauri::generate_handler![
                crate::trusted_welcome::welcome_status,
                crate::trusted_welcome::create_new_environment,
                crate::trusted_welcome::saved_environments,
                crate::trusted_welcome::activate_saved_environment,
                crate::trusted_welcome::trash_saved_environment,
                crate::trusted_welcome::constitute_personal_space,
                crate::packaged_navigator::open_space_navigator,
                application_session,
                runtime::dispatch_map_command::dispatch_map_command,
                crate::source_ingress::commands::select_loader_sources,
                crate::source_ingress::picker::source_picker_capabilities,
            ])
        };

        tracing::debug!("[APP BUILDER] Applying provider plugins.");
        let with_plugins =
            PluginManager::apply_plugins(base, &storage_manager, &runtime_selection, &registry)
                .unwrap_or_else(|e| {
                    tracing::error!("[APP BUILDER] failed to apply provider plugins: {}", e);
                    std::process::exit(1);
                });

        tracing::debug!("[APP BUILDER] Registering setup orchestration.");
        let with_registry = with_plugins.manage(registry);
        let builder = with_registry.setup(|app| -> Result<(), Box<dyn std::error::Error>> {
            let handle = app.handle().clone();
            if !crate::env::development_enabled() {
                // Welcome appears during provisioning; backend admission remains closed until ready.
                crate::trusted_welcome::create_window(&handle)?;
            }
            let storage_cfg = app.state::<StorageManager>().inner().clone();
            let runtime_selection = app.state::<ProviderRuntimeSelection>().inner().clone();

            tauri::async_runtime::spawn(async move {
                let startup_result = async {
                    let startup_started_at = Instant::now();
                    let setup_started_at = Instant::now();
                    SetupManager::apply_setups(&handle, &storage_cfg, &runtime_selection)
                        .await
                        .context("apply_setups failed")?;
                    tracing::info!(
                        "[PERF-688] conductora_startup: provider_setup_ms={}",
                        setup_started_at.elapsed().as_millis(),
                    );

                    let receptor_config_started_at = Instant::now();
                    AppBuilder::load_receptor_configs(&handle)
                        .await
                        .context("load_receptor_configs failed")?;
                    tracing::info!(
                        "[PERF-688] conductora_startup: receptor_config_ms={}",
                        receptor_config_started_at.elapsed().as_millis(),
                    );

                    let runtime_started_at = Instant::now();
                    if !runtime::init_from_state(&handle) {
                        anyhow::bail!("MAP Commands runtime initialization failed");
                    }
                    tracing::info!(
                        "[PERF-688] conductora_startup: runtime_init_ms={}",
                        runtime_started_at.elapsed().as_millis(),
                    );

                    let application_session = handle
                        .try_state::<ApplicationSessionState>()
                        .ok_or_else(|| anyhow::anyhow!("ApplicationSessionState is not managed"))?;
                    application_session.mark_opening_space().map_err(anyhow::Error::msg)?;
                    if crate::env::development_enabled() {
                        // The UI reports startup progress while semantic initialization runs.
                        // Its conductor integration and command runtime must already exist.
                        let window_started_at = Instant::now();
                        SetupManager::create_window(&handle, &storage_cfg, &runtime_selection)
                            .await
                            .context("create_window failed")?;
                        tracing::info!(
                            "[PERF-688] conductora_startup: window_creation_ms={}",
                            window_started_at.elapsed().as_millis(),
                        );
                    }
                    let bootstrap_started_at = Instant::now();
                    let startup = ensure_core_schema_space(&handle)
                        .await
                        .context("ensure_core_schema_space failed")?;
                    tracing::info!(
                        "[PERF-688] conductora_startup: core_schema_bootstrap_ms={}",
                        bootstrap_started_at.elapsed().as_millis(),
                    );
                    if startup
                        == crate::setup::core_schema_bootstrap::CoreSchemaStartup::Provisioned
                    {
                        application_session
                            .mark_activating_base_packages()
                            .map_err(anyhow::Error::msg)?;
                    }
                    ensure_configured_space_imports(&handle, startup)
                        .await
                        .context("ensure_configured_space_imports failed")?;
                    if !crate::env::development_enabled() {
                        crate::trusted_welcome::refresh_lifecycle(&handle).await?;
                        return Ok::<(), anyhow::Error>(());
                    }
                    let runtime = handle.state::<runtime::RuntimeState>().read()
                        .map_err(|e| anyhow::anyhow!(e.to_string()))?.clone()
                        .ok_or_else(|| anyhow::anyhow!("MAP runtime is unavailable"))?;
                    crate::setup::application_launcher::prepare_application_launch(&runtime, &application_session,
                        map_commands_runtime::application_launch::LaunchRequirement::ProvisionedSpace).await?;

                    tracing::info!(
                        "[PERF-688] conductora_startup: total_ms={}",
                        startup_started_at.elapsed().as_millis(),
                    );
                    handle
                        .emit(STARTUP_READY_EVENT, ())
                        .context("failed to emit startup ready event")?;
                    Ok::<(), anyhow::Error>(())
                }
                .await;

                if let Err(e) = startup_result {
                    if let Some(mismatch) = e.chain().find_map(|cause| {
                        match cause.downcast_ref::<tauri_plugin_holochain::HolochainRuntimeError>() {
                            Some(tauri_plugin_holochain::HolochainRuntimeError::IncompatibleEnvironment(message)) => Some(message.clone()),
                            _ => None,
                        }
                    }) {
                        if let Ok(mut state) = handle.state::<crate::config::environment::EnvironmentMismatch>().0.write() {
                            *state = Some(mismatch);
                        }
                    }
                    if let Some(application_session) = handle.try_state::<ApplicationSessionState>()
                    {
                        if let Err(session_error) = application_session.mark_failed(e.to_string()) {
                            tracing::error!(
                                "[APP BUILDER] failed to record application-session failure: {}",
                                session_error
                            );
                        }
                    }
                    if let Some(gate) = handle.try_state::<CoreSchemaBootstrapGate>() {
                        if let Err(gate_error) = gate.mark_failed() {
                            tracing::error!(
                                "[APP BUILDER] failed to close Core Schema bootstrap gate: {}",
                                gate_error
                            );
                        }
                    }
                    // eprintln flushes synchronously before process::exit kills async writers
                    eprintln!("[APP BUILDER] FATAL startup error: {:#}", e);
                    tracing::error!("[APP BUILDER] startup failed: {}", e);
                    if crate::env::development_enabled() {
                        std::process::exit(1);
                    }
                    // Keep packaged Welcome available to explain failure. Neither
                    // startup failure nor another launch may grant acquisition authority.
                    return;
                }
                tracing::info!("[APP BUILDER] App builder done, Startup complete.");
            });

            Ok(())
        });
        builder
    }

    async fn load_receptor_configs(handle: &AppHandle) -> anyhow::Result<()> {
        if let Some(registry) = handle.try_state::<ReceptorConfigRegistry>() {
            let configs = registry.all();
            if let Some(factory) = handle.try_state::<DeprecatedReceptorFactory>() {
                factory
                    .load_from_configs(configs)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to load receptor configs: {}", e))?;
                tracing::debug!("[APP BUILDER] ReceptorFactory loaded from configs.");
            }
        }
        Ok(())
    }
}

/// Returns the runtime-only session established by the MAP Application Launcher.
#[tauri::command]
fn application_session(
    window: tauri::WebviewWindow,
    application_session: tauri::State<'_, ApplicationSessionState>,
) -> Result<crate::setup::application_launcher::ApplicationSessionSnapshot, String> {
    crate::packaged_navigator::admit_operational_window(&window)?;
    application_session.snapshot()
}
