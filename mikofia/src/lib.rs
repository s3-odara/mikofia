use std::fs;
use std::io;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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

pub fn check(nodes: &[Node], root: &Path) -> Vec<Violation> {
    nodes
        .iter()
        .flat_map(|node| check_node(node, root))
        .collect()
}

/// Type state markers for compile-time step ordering
struct Initial;
struct ExistenceChecked;
struct KindChecked;

/// Pipeline for executing validation steps with type-safe ordering
struct CheckPipeline<'a, State> {
    node: &'a Node,
    path: PathBuf,
    exists: bool,
    violations: Vec<Violation>,
    should_continue: bool,
    _state: PhantomData<State>,
}

impl<'a> CheckPipeline<'a, Initial> {
    /// Create a new validation pipeline
    fn new(node: &'a Node, path: PathBuf, exists: bool) -> Self {
        Self {
            node,
            path,
            exists,
            violations: Vec::new(),
            should_continue: true,
            _state: PhantomData,
        }
    }

    /// Check existence requirements
    fn check_existence(mut self) -> CheckPipeline<'a, ExistenceChecked> {
        if !self.should_continue {
            return CheckPipeline {
                node: self.node,
                path: self.path,
                exists: self.exists,
                violations: self.violations,
                should_continue: false,
                _state: PhantomData,
            };
        }

        let violation = match self.node.existence {
            Existence::Required if !self.exists => Some(Violation {
                path: self.node.path.clone(),
                message: format!("Required item not found: {}", self.node.path),
            }),
            Existence::Absent if self.exists => Some(Violation {
                path: self.node.path.clone(),
                message: format!("Item must not exist: {}", self.node.path),
            }),
            _ => None,
        };

        if let Some(v) = violation {
            self.violations.push(v);
            self.should_continue = false;
        } else if !self.exists {
            self.should_continue = false;
        }

        CheckPipeline {
            node: self.node,
            path: self.path,
            exists: self.exists,
            violations: self.violations,
            should_continue: self.should_continue,
            _state: PhantomData,
        }
    }
}

impl<'a> CheckPipeline<'a, ExistenceChecked> {
    /// Check kind (file/directory)
    fn check_kind(mut self) -> CheckPipeline<'a, KindChecked> {
        if !self.should_continue {
            return CheckPipeline {
                node: self.node,
                path: self.path,
                exists: self.exists,
                violations: self.violations,
                should_continue: false,
                _state: PhantomData,
            };
        }

        let violation = match self.node.kind {
            NodeKind::File if !self.path.is_file() => Some(Violation {
                path: self.node.path.clone(),
                message: format!("Expected file, but found directory: {}", self.node.path),
            }),
            NodeKind::Directory if !self.path.is_dir() => Some(Violation {
                path: self.node.path.clone(),
                message: format!("Expected directory, but found file: {}", self.node.path),
            }),
            _ => None,
        };

        if let Some(v) = violation {
            self.violations.push(v);
            self.should_continue = false;
        }

        CheckPipeline {
            node: self.node,
            path: self.path,
            exists: self.exists,
            violations: self.violations,
            should_continue: self.should_continue,
            _state: PhantomData,
        }
    }
}

impl<'a> CheckPipeline<'a, KindChecked> {
    /// Check directory (strict + children)
    fn check_directory(mut self) -> Self {
        if !self.should_continue || !self.path.is_dir() {
            return self;
        }

        let strict_violations = check_strict(self.node, &self.path);
        let children_violations = check_children(self.node, &self.path);

        self.violations.extend(strict_violations);
        self.violations.extend(children_violations);

        self
    }

    /// Extract final violations from the pipeline
    fn violations(self) -> Vec<Violation> {
        self.violations
    }
}

fn check_node(node: &Node, root: &Path) -> Vec<Violation> {
    let full_path = root.join(&node.path);
    let exists = full_path.exists();

    CheckPipeline::new(node, full_path, exists)
        .check_existence()
        .check_kind()
        .check_directory()
        .violations()
}

fn check_children(node: &Node, dir_path: &Path) -> Vec<Violation> {
    node.children
        .iter()
        .flat_map(|child| check_node(child, dir_path))
        .collect()
}

fn check_strict(node: &Node, dir_path: &Path) -> Vec<Violation> {
    if !node.is_strict() {
        return vec![];
    }

    // Get actual items in directory
    let actual_items: Vec<String> = match fs::read_dir(dir_path) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect(),
        Err(_) => return vec![], // Ignore read errors
    };

    // Get defined children paths
    let defined_items: Vec<&str> = node
        .children
        .iter()
        .map(|child| child.path.as_str())
        .collect();

    // Find unlisted items
    actual_items
        .into_iter()
        .filter(|item| !defined_items.contains(&item.as_str()))
        .map(|item| Violation {
            path: format!("{}/{}", node.path, item),
            message: format!("Unlisted child item: {}", item),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_empty_nodes() {
        let nodes = vec![];
        let violations = check(&nodes, &PathBuf::from("."));
        assert!(violations.is_empty());
    }

    #[test]
    fn test_required_exists() {
        let nodes = vec![Node {
            path: "Cargo.toml".to_string(),
            existence: Existence::Required,
            kind: NodeKind::Any,
            children: vec![],
            strict: None,
        }];
        let violations = check(&nodes, &PathBuf::from("."));
        assert!(violations.is_empty());
    }

    #[test]
    fn test_optional_default() {
        assert_eq!(Existence::default(), Existence::Optional);
    }
}
