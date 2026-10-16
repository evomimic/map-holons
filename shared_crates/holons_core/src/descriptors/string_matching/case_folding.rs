use super::case_folding_table::CASE_FOLDING;

/// Applies Unicode full default case folding (CaseFolding.txt status C and F mappings).
///
/// The mapping version is fixed by the generated table rather than by the compiler's
/// standard library, so host and guest builds fold identically. No normalization,
/// locale tailoring, or Turkic mapping is applied; unmapped scalars pass through.
pub fn default_case_fold(text: &str) -> String {
    let mut folded = String::with_capacity(text.len());
    for scalar in text.chars() {
        match CASE_FOLDING.binary_search_by_key(&scalar, |(source, _)| *source) {
            Ok(index) => folded.push_str(CASE_FOLDING[index].1),
            Err(_) => folded.push(scalar),
        }
    }
    folded
}

#[cfg(test)]
mod tests {
    use super::super::CASE_FOLDING_UNICODE_VERSION;
    use super::*;

    #[test]
    fn table_is_pinned_to_unicode_17_and_sorted_for_binary_search() {
        assert_eq!(CASE_FOLDING_UNICODE_VERSION, "17.0.0");
        // CaseFolding-17.0.0.txt declares 1585 status C and F mappings.
        assert_eq!(CASE_FOLDING.len(), 1585);
        assert!(CASE_FOLDING.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }

    #[test]
    fn full_folding_expands_sharp_s_and_ligatures() {
        assert_eq!(default_case_fold("Straße"), "strasse");
        assert_eq!(default_case_fold("STRASSE"), "strasse");
        // Capital sharp s uses its F mapping, not the simple S mapping to ß.
        assert_eq!(default_case_fold("\u{1E9E}"), "ss");
        assert_eq!(default_case_fold("\u{FB01}"), "fi");
    }

    #[test]
    fn final_and_ordinary_sigma_fold_alike() {
        assert_eq!(default_case_fold("ΟΔΟΣ"), default_case_fold("οδος"));
        assert_eq!(default_case_fold("\u{03C2}"), "\u{03C3}");
    }

    #[test]
    fn dotted_capital_i_uses_the_default_mapping_not_turkic_tailoring() {
        assert_eq!(default_case_fold("\u{0130}"), "i\u{0307}");
        assert_eq!(default_case_fold("I"), "i");
    }

    #[test]
    fn cherokee_lowercase_folds_to_uppercase() {
        assert_eq!(default_case_fold("\u{AB70}"), "\u{13A0}");
        assert_eq!(default_case_fold("\u{13F8}"), "\u{13F0}");
    }

    #[test]
    fn unmapped_scalars_and_decompositions_pass_through_unchanged() {
        assert_eq!(default_case_fold("café 42 ☃"), "café 42 ☃");
        assert_eq!(default_case_fold("e\u{0301}"), "e\u{0301}");
        assert_eq!(default_case_fold(""), "");
    }
}
