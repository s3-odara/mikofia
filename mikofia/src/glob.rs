use globset::{Glob, GlobBuilder, escape};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use crate::fs::FileSystem;

/// Errors that can occur during glob expansion
#[derive(Debug)]
pub enum GlobError {
    InvalidPattern { pattern: String, details: String },
    Walk { base: PathBuf, error: io::Error },
}

impl fmt::Display for GlobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GlobError::InvalidPattern { pattern, details } => {
                write!(f, "Invalid glob pattern '{}': {}", pattern, details)
            }
            GlobError::Walk { base, error } => {
                write!(
                    f,
                    "Failed to walk directory '{}': {}",
                    base.display(),
                    error
                )
            }
        }
    }
}

impl std::error::Error for GlobError {}

/// Expand a glob pattern relative to a base directory
/// Returns a list of paths that match the pattern
pub fn expand_glob<F: FileSystem>(
    pattern: &str,
    base: &Path,
    fs: &F,
) -> Result<Vec<PathBuf>, GlobError> {
    // Compile the glob pattern with literal separators so `*` does not cross directories
    let glob = build_literal_glob(pattern).map_err(|e| GlobError::InvalidPattern {
        pattern: pattern.to_string(),
        details: e.to_string(),
    })?;
    let matcher = glob.compile_matcher();

    // Walk the directory and collect matching paths
    let matches = fs
        .walk_dir(base, |path| {
            // Get path relative to base
            if let Ok(rel_path) = path.strip_prefix(base) {
                // Convert to string for matching
                if let Some(path_str) = rel_path.to_str() {
                    return matcher.is_match(path_str);
                }
            }
            false
        })
        .map_err(|error| GlobError::Walk {
            base: base.to_path_buf(),
            error,
        })?;

    Ok(matches)
}

/// Determine whether a pattern contains glob syntax recognized by `globset`.
pub fn is_glob_pattern(pattern: &str) -> bool {
    let parsed = match GlobBuilder::new(pattern).build() {
        Ok(glob) => glob,
        Err(_) => return false,
    };

    let literal_pattern = escape(pattern);
    let literal = match GlobBuilder::new(&literal_pattern).build() {
        Ok(glob) => glob,
        Err(_) => return false,
    };

    parsed.regex() != literal.regex()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::mock::MockFileSystem;
    use std::cell::Cell;
    use std::io;

    #[test]
    fn expand_glob_returns_matching_paths() {
        let base = PathBuf::from("/project");
        let mut fs = MockFileSystem::new();
        fs.add_dir(base.clone());
        fs.add_dir(base.join("src"));
        fs.add_file(base.join("src/lib.rs"), "content");
        fs.add_file(base.join("src/main.rs"), "content");
        fs.add_file(base.join("tests/test.rs"), "content");

        let matches = expand_glob("src/*.rs", &base, &fs).expect("should expand glob");
        let mut paths: Vec<_> = matches
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        paths.sort();

        assert_eq!(
            paths,
            vec![
                "/project/src/lib.rs".to_string(),
                "/project/src/main.rs".to_string()
            ]
        );
    }

    #[test]
    fn expand_glob_fails_on_invalid_pattern() {
        let base = Path::new("/project");
        let fs = MockFileSystem::new();

        let err = expand_glob("[", base, &fs).expect_err("invalid glob should error");
        match err {
            GlobError::InvalidPattern { pattern, .. } => assert_eq!(pattern, "["),
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    struct ErrorFs {
        read_error_emitted: Cell<bool>,
    }

    impl ErrorFs {
        fn new() -> Self {
            Self {
                read_error_emitted: Cell::new(false),
            }
        }
    }

    impl FileSystem for ErrorFs {
        fn exists(&self, _path: &Path) -> bool {
            false
        }

        fn is_dir(&self, _path: &Path) -> bool {
            false
        }

        fn read_dir(&self, _path: &Path) -> io::Result<Vec<String>> {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "read_dir should not be called",
            ))
        }

        fn read_to_string(&self, _path: &Path) -> io::Result<String> {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "read_to_string should not be called",
            ))
        }

        fn walk_dir<F>(&self, _path: &Path, _predicate: F) -> io::Result<Vec<PathBuf>>
        where
            F: Fn(&Path) -> bool,
        {
            self.read_error_emitted.set(true);
            Err(io::Error::new(
                io::ErrorKind::Other,
                "simulated walk_dir failure",
            ))
        }
    }

    #[test]
    fn expand_glob_propagates_walk_dir_errors() {
        let base = Path::new("/project");
        let fs = ErrorFs::new();

        let err = expand_glob("*.rs", base, &fs).expect_err("walk_dir error should surface");
        match err {
            GlobError::Walk { error, .. } => {
                assert_eq!(error.kind(), io::ErrorKind::Other);
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
        assert!(fs.read_error_emitted.get());
    }

    #[test]
    fn single_star_does_not_cross_directory_separator() {
        let mut fs = MockFileSystem::new();
        let base = PathBuf::from("/project");
        fs.add_dir(base.clone());
        fs.add_dir(base.join("subdir"));
        fs.add_file(base.join("subdir/file.txt"), "content");
        fs.add_file(base.join("subdir/nested/file.txt"), "content");

        let glob = build_literal_glob("subdir/*").expect("should compile");
        let matcher = glob.compile_matcher();
        assert!(matcher.is_match("subdir/file.txt"));
        assert!(!matcher.is_match("subdir/nested/file.txt"));
    }

    #[test]
    fn detects_glob_patterns() {
        assert!(is_glob_pattern("*.rs"));
        assert!(is_glob_pattern("src/**/mod.rs"));
        assert!(is_glob_pattern("{foo,bar}.txt"));
    }

    #[test]
    fn treats_literals_as_non_glob() {
        assert!(!is_glob_pattern("src/main.rs"));
        assert!(!is_glob_pattern("node_modules"));
        assert!(!is_glob_pattern("foo\u{2603}.txt"));
    }
}

/// Build a glob pattern that treats path separators literally (similar to minimatch).
pub fn build_literal_glob(pattern: &str) -> Result<Glob, globset::Error> {
    GlobBuilder::new(pattern).literal_separator(true).build()
}
