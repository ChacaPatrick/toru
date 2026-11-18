/// `query` is already lower-cased and trimmed.
pub(super) fn name_matches(name: &str, query: &str) -> bool {
    query.is_empty() || name.to_lowercase().contains(query)
}

pub(super) fn no_match_text(query: &str, elsewhere: bool) -> String {
    if elsewhere {
        format!("Nothing in this session matches “{query}”")
    } else {
        format!("Nothing matches “{query}”")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_ignores_case_and_an_empty_query_matches_all() {
        assert!(name_matches("Login-Bug-Repro", "bug"));
        assert!(!name_matches("Login-Bug-Repro", "crash"));
        assert!(name_matches("anything", ""));
    }

    #[test]
    fn no_match_notice_points_at_earlier_groups() {
        assert_eq!(
            no_match_text("bug", true),
            "Nothing in this session matches “bug”"
        );
        assert_eq!(no_match_text("bug", false), "Nothing matches “bug”");
    }
}
