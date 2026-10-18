mod case_folding;
mod case_folding_table;
mod case_matching;

pub use case_folding::default_case_fold;
pub use case_folding_table::CASE_FOLDING_UNICODE_VERSION;
pub use case_matching::CaseMatching;
