use globset::Glob;
use std::path::{Path, PathBuf};

use crate::fs::FileSystem;

/// Expand a glob pattern relative to a base directory
/// Returns a list of paths that match the pattern
pub(crate) fn expand_glob<F: FileSystem>(
    pattern: &str,
    base: &Path,
    fs: &F,
) -> Result<Vec<PathBuf>, String> {
    // Compile the glob pattern
    let glob = Glob::new(pattern)
        .map_err(|e| format!("Invalid glob pattern '{}': {}", pattern, e))?;
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
        .map_err(|e| format!("Failed to walk directory: {}", e))?;

    Ok(matches)
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
        let mut paths: Vec<_> = matches.iter().map(|p| p.to_string_lossy().into_owned()).collect();
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
        assert!(
            err.contains("Invalid glob pattern '['"),
            "unexpected error message: {err}"
        );
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
        assert!(
            err.contains("Failed to walk directory"),
            "unexpected error message: {err}"
        );
        assert!(fs.read_error_emitted.get());
    }
}
