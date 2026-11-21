//! Connect Welcome to the runtime and startup state owned by the application shell.
use tauri::Manager;

pub fn host_services() -> welcome::host::HostServices {
    welcome::host::HostServices {
        startup: |handle| {
            let application =
                handle.state::<super::application_launcher::ApplicationSessionState>();
            Ok((application.snapshot()?.phase, application.preparing_new_space()?))
        },
        runtime: |handle| {
            handle
                .state::<crate::runtime::RuntimeState>()
                .read()
                .map_err(|error| error.to_string())?
                .clone()
                .ok_or_else(|| "MAP runtime is unavailable".into())
        },
        environment_base: crate::config::environment::packaged_base,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn welcome_permissions_exclude_conductor_and_generic_map_commands() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../../capabilities/welcome.json")).unwrap();
        assert_eq!(capability["webviews"], serde_json::json!(["welcome"]));
        assert!(capability.get("windows").is_none());
        assert!(capability.get("remote").is_none());
        assert_eq!(
            capability["permissions"],
            serde_json::json!([
                "allow-welcome-status",
                "allow-create-new-environment",
                "allow-saved-environments",
                "allow-activate-saved-environment",
                "allow-trash-saved-environment",
                "allow-constitute-personal-space",
                "allow-open-space-navigator"
            ])
        );
    }
}
