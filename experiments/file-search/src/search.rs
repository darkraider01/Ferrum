use crate::scanner::ScannedEntry;

/// Filters scanned entries by query using a documented case-insensitive substring rule
/// and returns matching indices sorted stably by relative path.
///
/// Search semantics:
/// 1. An empty or whitespace-only query matches all entries.
/// 2. If the query contains `/`, matching is performed against the full relative path.
/// 3. Otherwise, matching checks if the base filename contains the query substring (case-insensitive).
/// 4. Results are returned in stable lexicographical order of `relative_path`.
pub fn filter_matches(entries: &[ScannedEntry], query: &str) -> Vec<usize> {
    let trimmed = query.trim();

    let mut matches: Vec<usize> = if trimmed.is_empty() {
        (0..entries.len()).collect()
    } else {
        let lower_query = trimmed.to_lowercase();
        let query_has_separator = lower_query.contains('/') || lower_query.contains('\\');

        entries
            .iter()
            .enumerate()
            .filter_map(|(idx, entry)| {
                let matches = if query_has_separator {
                    entry.relative_path.to_lowercase().contains(&lower_query)
                } else {
                    entry.filename.to_lowercase().contains(&lower_query)
                };

                if matches {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect()
    };

    // Stable sort by relative path
    matches.sort_by(|&a, &b| entries[a].relative_path.cmp(&entries[b].relative_path));

    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(rel: &str, file: &str) -> ScannedEntry {
        ScannedEntry {
            relative_path: rel.to_string(),
            filename: file.to_string(),
        }
    }

    #[test]
    fn test_empty_query_matches_all_in_stable_order() {
        let entries = vec![
            make_entry("src/main.rs", "main.rs"),
            make_entry("Cargo.toml", "Cargo.toml"),
            make_entry("src/lib.rs", "lib.rs"),
        ];

        let matches = filter_matches(&entries, "");
        assert_eq!(matches.len(), 3);
        // Sorted lexicographically: Cargo.toml (1), src/lib.rs (2), src/main.rs (0)
        assert_eq!(matches, vec![1, 2, 0]);
    }

    #[test]
    fn test_case_insensitive_filename_match() {
        let entries = vec![
            make_entry("README.md", "README.md"),
            make_entry("src/readme_helper.rs", "readme_helper.rs"),
            make_entry("src/main.rs", "main.rs"),
        ];

        let matches = filter_matches(&entries, "readme");
        assert_eq!(matches.len(), 2);
        assert_eq!(matches, vec![0, 1]); // README.md, src/readme_helper.rs
    }

    #[test]
    fn test_path_separator_in_query() {
        let entries = vec![
            make_entry("src/scanner/mod.rs", "mod.rs"),
            make_entry("tests/scanner_test.rs", "scanner_test.rs"),
            make_entry("src/main.rs", "main.rs"),
        ];

        let matches = filter_matches(&entries, "scanner/");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches, vec![0]);
    }

    #[test]
    fn test_no_matches() {
        let entries = vec![
            make_entry("src/main.rs", "main.rs"),
            make_entry("Cargo.toml", "Cargo.toml"),
        ];

        let matches = filter_matches(&entries, "nonexistent");
        assert!(matches.is_empty());
    }
}
