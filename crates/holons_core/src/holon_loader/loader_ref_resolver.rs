
use std::collections::HashMap;
use base_types::MapString;
use core_types::HolonError;
use crate::reference_layer::{HolonsContextBehavior, TransientReference};
use crate::StagedReference;

pub struct ResolverOutcome {
    pub links_created: usize,
    pub errors: Vec<HolonError>,
}

pub struct LoaderRefResolver;

impl LoaderRefResolver {
    pub fn resolve_relationships(
        context: &dyn HolonsContextBehavior,
        key_index: &HashMap<MapString, StagedReference>,
        queued_rel_refs: Vec<TransientReference>,
    ) -> Result<ResolverOutcome, HolonError> {
        // resolve LoaderRelationshipReference holons and add declared links (incl. DescribedBy)
        unimplemented!()
    }
}
