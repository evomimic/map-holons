//use dotenvy::from_path_iter;
//use std::path::Path;

fn main() {
    // Load .env at compile time
    //let env_path = Path::new(".env");
    //if env_path.exists() {
    //    for item in from_path_iter(env_path).expect("Failed to parse .env") {
    //         let (key, value) = item.expect("Invalid .env entry");
    //        println!("cargo:rustc-env={}={}", key, value);
    //     }
    // }

    // You can also set rerun-if-changed
    //println!("cargo:rerun-if-changed=.env");
    let commands = &[
        "application_session",
        "select_loader_sources",
        "source_picker_capabilities",
        "root_space",
        "serde_test",
        "map_request",
        "all_spaces",
        "is_service_ready",
        "dispatch_map_command",
        "welcome_status",
        "create_new_environment",
        "open_space_navigator",
        "constitute_personal_space",
    ];
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(commands)),
    )
    .expect("Tauri build configuration");
}
