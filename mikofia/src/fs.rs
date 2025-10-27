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
        Ok(entries
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect())
    }

    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        std::fs::read_to_string(path)
    }

    fn walk_dir<F>(&self, path: &Path, predicate: F) -> io::Result<Vec<PathBuf>>
    where
        F: Fn(&Path) -> bool,
    {
        use walkdir::WalkDir;

        let matches: Vec<PathBuf> = WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|entry| predicate(entry.path()))
            .map(|entry| entry.path().to_path_buf())
            .collect();

        Ok(matches)
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::collections::HashMap;

    /// Mock filesystem for testing
    #[derive(Debug, Clone, Default)]
    pub struct MockFileSystem {
        files: HashMap<PathBuf, String>,
        directories: Vec<PathBuf>,
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
