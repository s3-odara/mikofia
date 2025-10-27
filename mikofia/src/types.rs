use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

use crate::fs::{FileSystem, RealFileSystem};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Node {
    pub path: String,

    #[serde(default)]
    pub existence: Existence,

    #[serde(default)]
    pub kind: NodeKind,

    #[serde(default)]
    pub children: Vec<Node>,

    #[serde(default)]
    pub strict: Option<bool>,
}

impl Node {
    /// Determine if strict mode is enabled
    /// If strict is explicitly set, use that value
    /// Otherwise, default to true if children exist, false if not
    pub fn is_strict(&self) -> bool {
        self.strict.unwrap_or_else(|| !self.children.is_empty())
    }

    /// Check whether the path uses glob syntax understood by `globset`
    pub fn is_glob_pattern(&self) -> bool {
        use globset::{GlobBuilder, escape};

        // If the pattern fails to parse as a glob, treat it as a literal.
        let Ok(parsed) = GlobBuilder::new(&self.path).build() else {
            return false;
        };

        // Compare against the same text escaped into a purely literal glob.
        let literal_pattern = escape(&self.path);
        let Ok(literal) = GlobBuilder::new(&literal_pattern).build() else {
            return false;
        };

        parsed.regex() != literal.regex()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Existence {
    Required,
    #[default]
    Optional,
    Absent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    File,
    Directory,
    #[default]
    Any,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub nodes: Vec<Node>,
}

impl Config {
    /// Load configuration from a JSON file
    pub fn from_file(path: &Path) -> io::Result<Self> {
        Self::from_file_with_fs(path, &RealFileSystem)
    }

    /// Load configuration from a JSON file with custom filesystem
    pub fn from_file_with_fs<F: FileSystem>(path: &Path, fs: &F) -> io::Result<Self> {
        let content = fs.read_to_string(path)?;
        let config: Config = serde_json::from_str(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(config)
    }
}

#[derive(Debug)]
pub struct Violation {
    pub path: String,
    pub message: String,
}
