//! Integration checks that share the runtime matrix's bootstrap and persisted setup.
mod attachment_defaults;
mod key_uniqueness;
mod relationship_cache_policy;
mod space_dancer_import;
mod visualizer_composition;

use map_commands_runtime::Runtime;

pub async fn run(runtime: &Runtime) {
    macro_rules! check {
        ($module:ident, $name:ident) => {{
            let started = std::time::Instant::now();
            tracing::info!(check = stringify!($name), "runtime check starting");
            $module::$name(runtime).await;
            tracing::info!(
                check = stringify!($name),
                elapsed_ms = started.elapsed().as_millis(),
                "runtime check passed"
            );
        }};
    }
    check!(relationship_cache_policy, configured_inverse_policies_retain_committed_membership);
    check!(
        visualizer_composition,
        saved_visualizer_composition_stops_before_generic_descriptor_ancestors
    );
    check!(
        attachment_defaults,
        mixed_import_promotes_saved_descriptor_and_materializes_early_enum_defaults
    );
    check!(key_uniqueness, reimporting_domain_creates_is_a_key_uniqueness_rejection);
    check!(space_dancer_import, importing_space_navigator_affords_its_dancer_from_the_local_space);
}
