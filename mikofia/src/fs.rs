use std::io;
use std::path::{Path, PathBuf};

/// Abstraction for filesystem operations to enable testing
pub trait FileSystem {
    /// Check if a path exists
    fn exists(&self, path: &Path) -> bool;

    /// Check if a path is a directory
    fn is_dir(&self, path: &Path) -> bool;

    /// Read directory entries
    fn read_dir(&self, path: &Path) -> io::Result<Vec<String>>;

    /// Read file contents as string
    fn read_to_string(&self, path: &Path) -> io::Result<String>;

    /// Walk directory tree and collect paths matching a predicate
    fn walk_dir<F>(&self, path: &Path, predicate: F) -> io::Result<Vec<PathBuf>>
    where
        F: Fn(&Path) -> bool;
}

/// Determine if an I/O error represents a permission problem
pub fn is_permission_denied(error: &io::Error) -> bool {
    matches!(error.kind(), io::ErrorKind::PermissionDenied)
}

/// Human-readable guidance for permission failures
pub fn permission_denied_message(path: &Path) -> String {
    format!(
        "Cannot access {} due to insufficient permissions. Grant access or rerun with elevated rights.",
        path.display()
    )
}

/// Real filesystem implementation
#[derive(Debug, Clone, Copy, Default)]
pub struct RealFileSystem;

impl FileSystem for RealFileSystem {
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn read_dir(&self, path: &Path) -> io::Result<Vec<String>> {
        let entries = std::fs::read_dir(path)?;
        entries
            .map(|entry| {
                let entry = entry?;
                entry
                    .file_name()
                    .into_string()
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid unicode"))
            })
            .collect()
    }

    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        std::fs::read_to_string(path)
    }

    fn walk_dir<F>(&self, path: &Path, predicate: F) -> io::Result<Vec<PathBuf>>
    where
        F: Fn(&Path) -> bool,
    {
        use walkdir::WalkDir;

        let mut matches: Vec<PathBuf> = Vec::new();

        for entry in WalkDir::new(path) {
            match entry {
                Ok(entry) => {
                    let entry_path = entry.path();
                    if predicate(entry_path) {
                        matches.push(entry_path.to_path_buf());
                    }
                }
                Err(err) => {
                    if let Some(io_err) = err.io_error()
                        && io_err.kind() == io::ErrorKind::PermissionDenied
                    {
                        let denied_path = err
                            .path()
                            .map(|p| p.to_path_buf())
                            .unwrap_or_else(|| path.to_path_buf());

                        return Err(io::Error::new(
                            io::ErrorKind::PermissionDenied,
                            format!(
                                "Permission denied while accessing {}",
                                denied_path.display()
                            ),
                        ));
                    }
                }
            }
        }

        Ok(matches)
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::collections::{HashMap, HashSet};

    /// Mock filesystem for testing
    #[derive(Debug, Clone, Default)]
    pub struct MockFileSystem {
        files: HashMap<PathBuf, String>,
        directories: Vec<PathBuf>,
        restricted_paths: HashSet<PathBuf>,
    }

    impl MockFileSystem {
        pub fn new() -> Self {
            Self::default()
        }

        /// Add a file with content
        pub fn add_file(&mut self, path: impl Into<PathBuf>, content: impl Into<String>) {
            self.files.insert(path.into(), content.into());
        }

        /// Add a directory
        pub fn add_dir(&mut self, path: impl Into<PathBuf>) {
            self.directories.push(path.into());
        }

        /// Mark a directory as inaccessible to simulate permission errors
        pub fn deny_dir(&mut self, path: impl Into<PathBuf>) {
            self.restricted_paths.insert(path.into());
        }
    }

    impl FileSystem for MockFileSystem {
        fn exists(&self, path: &Path) -> bool {
            let path_buf = path.to_path_buf();
            self.files.contains_key(&path_buf) || self.directories.contains(&path_buf)
        }

        fn is_dir(&self, path: &Path) -> bool {
            self.directories.contains(&path.to_path_buf())
        }

        fn read_dir(&self, path: &Path) -> io::Result<Vec<String>> {
            let path_buf = path.to_path_buf();

            if self.restricted_paths.contains(&path_buf) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("Permission denied: {}", path.display()),
                ));
            }

            let mut children: Vec<String> = self
                .files
                .keys()
                .chain(self.directories.iter())
                .filter_map(|p| {
                    p.parent().and_then(|parent| {
                        if parent == path {
                            p.file_name()
                                .and_then(|name| name.to_str())
                                .map(|s| s.to_string())
                        } else {
                            None
                        }
                    })
                })
                .collect();

            // Remove duplicates
            children.sort();
            children.dedup();

            if children.is_empty() && !self.directories.contains(&path_buf) {
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "Directory not found",
                ))
            } else {
                Ok(children)
            }
        }

        fn read_to_string(&self, path: &Path) -> io::Result<String> {
            self.files
                .get(path)
                .cloned()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "File not found"))
        }

        fn walk_dir<F>(&self, base: &Path, predicate: F) -> io::Result<Vec<PathBuf>>
        where
            F: Fn(&Path) -> bool,
        {
            let denied_path = self
                .restricted_paths
                .iter()
                .find(|restricted| restricted.starts_with(base) || base.starts_with(restricted))
                .cloned();

            if let Some(path) = denied_path {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("Permission denied: {}", path.display()),
                ));
            }

            let matches: Vec<PathBuf> = self
                .files
                .keys()
                .chain(self.directories.iter())
                .filter(|path| path.starts_with(base) && predicate(path))
                .cloned()
                .collect();

            Ok(matches)
        }
    }
}
