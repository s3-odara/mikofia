use globset::{GlobSet, GlobSetBuilder};
use std::path::Path;

/// Matcher for ignore patterns
#[derive(Debug, Clone)]
pub struct IgnoreMatcher {
    glob_set: GlobSet,
    patterns: Vec<String>,
}

impl IgnoreMatcher {
    /// Create a new IgnoreMatcher from a list of patterns
    pub fn new(patterns: &[String]) -> Result<Self, String> {
        let glob_set = patterns
            .iter()
            .try_fold(GlobSetBuilder::new(), |mut builder, pattern| {
                // Add the pattern itself
                let glob = crate::glob::build_literal_glob(pattern)
                    .map_err(|e| format!("Invalid ignore pattern '{}': {}", pattern, e))?;
                builder.add(glob);

                // For patterns without wildcards, also add a pattern that matches subdirectories
                // This makes "node_modules" match both "node_modules" and "node_modules/**/*"
                if !crate::glob::is_glob_pattern(pattern) {
                    let dir_pattern = format!("{}/**", pattern);
                    if let Ok(dir_glob) = crate::glob::build_literal_glob(&dir_pattern) {
                        builder.add(dir_glob);
                    }
                }

                Ok::<_, String>(builder)
            })?
            .build()
            .map_err(|e| format!("Failed to build ignore matcher: {}", e))?;

        Ok(Self {
            glob_set,
            patterns: patterns.to_vec(),
        })
    }

    /// Create an empty IgnoreMatcher that matches nothing
    pub fn empty() -> Self {
        Self {
            glob_set: GlobSetBuilder::new().build().unwrap(),
            patterns: Vec::new(),
        }
    }

    /// Create a new IgnoreMatcher from multiple pattern sets
    /// This combines all pattern sets into a single matcher
    pub fn from_multiple(pattern_sets: &[&[String]]) -> Result<Self, String> {
        let all_patterns: Vec<String> = pattern_sets
            .iter()
            .flat_map(|set| set.iter().cloned())
            .collect();

        Self::new(&all_patterns)
    }

    /// Get the patterns used by this matcher
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }

    /// Check if a path should be ignored
    pub fn is_ignored(&self, path: &Path) -> bool {
        self.glob_set.is_match(path)
    }

    /// Check if a path (as string) should be ignored
    pub fn is_ignored_str(&self, path: &str) -> bool {
        self.glob_set.is_match(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_matcher() {
        let matcher = IgnoreMatcher::empty();
        assert!(!matcher.is_ignored(Path::new("foo")));
        assert!(!matcher.is_ignored(Path::new("node_modules")));
    }

    #[test]
    fn test_exact_match() {
        let patterns = vec!["node_modules".to_string(), "target".to_string()];
        let matcher = IgnoreMatcher::new(&patterns).unwrap();

        assert!(matcher.is_ignored(Path::new("node_modules")));
        assert!(matcher.is_ignored(Path::new("target")));
        assert!(!matcher.is_ignored(Path::new("src")));
    }

    #[test]
    fn test_glob_pattern() {
        let patterns = vec!["*.log".to_string(), "**/.DS_Store".to_string()];
        let matcher = IgnoreMatcher::new(&patterns).unwrap();

        assert!(matcher.is_ignored(Path::new("app.log")));
        assert!(matcher.is_ignored(Path::new("error.log")));
        assert!(matcher.is_ignored(Path::new(".DS_Store")));
        assert!(matcher.is_ignored(Path::new("src/.DS_Store")));
        assert!(matcher.is_ignored(Path::new("src/components/.DS_Store")));
        assert!(!matcher.is_ignored(Path::new("src/main.rs")));
    }

    #[test]
    fn test_nested_paths() {
        let patterns = vec!["node_modules".to_string()];
        let matcher = IgnoreMatcher::new(&patterns).unwrap();

        assert!(matcher.is_ignored(Path::new("node_modules")));
        assert!(matcher.is_ignored(Path::new("node_modules/package")));
        assert!(!matcher.is_ignored(Path::new("src/node_modules")));
    }

    #[test]
    fn test_directory_pattern() {
        let patterns = vec!["**/node_modules".to_string(), "**/target".to_string()];
        let matcher = IgnoreMatcher::new(&patterns).unwrap();

        assert!(matcher.is_ignored(Path::new("node_modules")));
        assert!(matcher.is_ignored(Path::new("src/node_modules")));
        assert!(matcher.is_ignored(Path::new("foo/bar/node_modules")));
        assert!(matcher.is_ignored(Path::new("target")));
        assert!(matcher.is_ignored(Path::new("rust/target")));
        assert!(!matcher.is_ignored(Path::new("src")));
    }

    #[test]
    fn test_invalid_pattern() {
        let patterns = vec!["[invalid".to_string()];
        let result = IgnoreMatcher::new(&patterns);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_ignored_str() {
        let patterns = vec!["*.log".to_string()];
        let matcher = IgnoreMatcher::new(&patterns).unwrap();

        assert!(matcher.is_ignored_str("app.log"));
        assert!(matcher.is_ignored_str("error.log"));
        assert!(!matcher.is_ignored_str("main.rs"));
    }

    #[test]
    fn test_from_multiple_combines_patterns() {
        let global_patterns = vec!["node_modules".to_string(), "target".to_string()];
        let local_patterns = vec!["*.log".to_string(), "temp".to_string()];

        let matcher =
            IgnoreMatcher::from_multiple(&[&global_patterns, &local_patterns]).unwrap();

        // Global patterns should work
        assert!(matcher.is_ignored(Path::new("node_modules")));
        assert!(matcher.is_ignored(Path::new("target")));

        // Local patterns should work
        assert!(matcher.is_ignored(Path::new("app.log")));
        assert!(matcher.is_ignored(Path::new("temp")));

        // Non-matching paths
        assert!(!matcher.is_ignored(Path::new("src")));
        assert!(!matcher.is_ignored(Path::new("main.rs")));
    }

    #[test]
    fn test_from_multiple_with_empty_sets() {
        let patterns1 = vec!["*.log".to_string()];
        let patterns2: Vec<String> = vec![];

        let matcher = IgnoreMatcher::from_multiple(&[&patterns1, &patterns2]).unwrap();

        assert!(matcher.is_ignored(Path::new("app.log")));
        assert!(!matcher.is_ignored(Path::new("main.rs")));
    }

    #[test]
    fn test_from_multiple_all_empty() {
        let patterns1: Vec<String> = vec![];
        let patterns2: Vec<String> = vec![];

        let matcher = IgnoreMatcher::from_multiple(&[&patterns1, &patterns2]).unwrap();

        assert!(!matcher.is_ignored(Path::new("anything")));
    }

    #[test]
    fn test_patterns_method_returns_original_patterns() {
        let patterns = vec!["*.log".to_string(), "node_modules".to_string()];
        let matcher = IgnoreMatcher::new(&patterns).unwrap();

        assert_eq!(matcher.patterns(), &patterns);
    }
}
