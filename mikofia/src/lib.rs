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

fn check_node(node: &Node, root: &Path) -> Vec<Violation> {
    let full_path = root.join(&node.path);
    let exists = full_path.exists();

    // Check existence first
    let existence_violation = check_existence(node, exists);
    if existence_violation.is_some() || !exists {
        return existence_violation.into_iter().collect();
    }

    // Check kind only if item exists and passes existence check
    check_kind(node, &full_path).into_iter().collect()
}

fn check_existence(node: &Node, exists: bool) -> Option<Violation> {
    match node.existence {
        Existence::Required if !exists => Some(Violation {
            path: node.path.clone(),
            message: format!("Required item not found: {}", node.path),
        }),
        Existence::Absent if exists => Some(Violation {
            path: node.path.clone(),
            message: format!("Item must not exist: {}", node.path),
        }),
        _ => None,
    }
}

fn check_kind(node: &Node, full_path: &Path) -> Option<Violation> {
    match node.kind {
        NodeKind::File if !full_path.is_file() => Some(Violation {
            path: node.path.clone(),
            message: format!("Expected file, but found directory: {}", node.path),
        }),
        NodeKind::Directory if !full_path.is_dir() => Some(Violation {
            path: node.path.clone(),
            message: format!("Expected directory, but found file: {}", node.path),
        }),
        _ => None,
    }
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
        }];
        let violations = check(&nodes, &PathBuf::from("."));
        assert!(violations.is_empty());
    }

    #[test]
    fn test_optional_default() {
        assert_eq!(Existence::default(), Existence::Optional);
    }
}
