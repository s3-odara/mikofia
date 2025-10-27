use globset::Glob;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Expand a glob pattern relative to a base directory
/// Returns a list of paths that match the pattern
pub(crate) fn expand_glob(pattern: &str, base: &Path) -> Result<Vec<PathBuf>, String> {
    // Compile the glob pattern
    let glob = Glob::new(pattern)
        .map_err(|e| format!("Invalid glob pattern '{}': {}", pattern, e))?;
    let matcher = glob.compile_matcher();

    // Walk the directory and collect matching paths
    let matches: Vec<PathBuf> = WalkDir::new(base)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|entry| {
            // Get path relative to base
            if let Ok(rel_path) = entry.path().strip_prefix(base) {
                // Convert to string for matching
                if let Some(path_str) = rel_path.to_str() {
                    return matcher.is_match(path_str);
                }
            }
            false
        })
        .map(|entry| entry.path().to_path_buf())
        .collect();

    Ok(matches)
}

