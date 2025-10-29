use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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

    #[serde(skip)]
    pub rules: Vec<RuleHandle>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Violation {
    pub key: String,
    pub path: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<HashMap<String, serde_json::Value>>,
}

impl Violation {
    /// Create a new violation with key, path, and message
    pub fn new(key: impl Into<String>, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            path: path.into(),
            message: message.into(),
            args: None,
        }
    }

    /// Create a new violation with additional args
    pub fn with_args(
        key: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
        args: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            key: key.into(),
            path: path.into(),
            message: message.into(),
            args: Some(args),
        }
    }
}

/// Context provided to custom rules during evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationContext {
    pub path: String,
    pub name: String,
    pub extension: Option<String>,
    pub parent: Option<ParentInfo>,
    pub siblings: Vec<SiblingInfo>,
}

/// Information about a parent directory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParentInfo {
    pub path: String,
    pub name: String,
}

/// Information about a sibling file or directory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiblingInfo {
    pub name: String,
    pub is_file: bool,
    pub is_directory: bool,
}

/// Handle to a validation rule (native or JavaScript)
#[derive(Debug, Clone)]
pub enum RuleHandle {
    Native(std::sync::Arc<dyn NativeRule>),
    // JavaScript rule handle will be added by mikofia-deno
}

/// Trait for native validation rules
pub trait NativeRule: Send + Sync + std::fmt::Debug {
    fn check(&self, ctx: &EvaluationContext) -> RuleResult;
}

/// Result of a rule evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RuleResult {
    Pass,
    Fail { violation: Violation },
    Skip { reason: String },
}
