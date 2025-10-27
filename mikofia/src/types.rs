use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

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

    /// Check if path contains glob pattern characters
    pub fn is_glob_pattern(&self) -> bool {
        self.path.contains('*') || self.path.contains('?') || self.path.contains('[')
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
        let content = fs::read_to_string(path)?;
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
