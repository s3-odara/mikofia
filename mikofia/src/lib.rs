use std::path::Path;

#[derive(Debug)]
pub struct Node {
    pub path: String,
    pub existence: Existence,
    pub kind: NodeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Existence {
    Required,
    #[default]
    Optional,
    Absent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeKind {
    File,
    Directory,
    #[default]
    Any,
}

#[derive(Debug)]
pub struct Violation {
    pub path: String,
    pub message: String,
}

pub fn check(nodes: &[Node], root: &Path) -> Vec<Violation> {
    let mut violations = Vec::new();

    for node in nodes {
        let full_path = root.join(&node.path);
        let exists = full_path.exists();

        match node.existence {
            Existence::Required => {
                if !exists {
                    violations.push(Violation {
                        path: node.path.clone(),
                        message: format!("Required item not found: {}", node.path),
                    });
                    continue;
                }
            }
            Existence::Optional => {
                if !exists {
                    continue;
                }
            }
            Existence::Absent => {
                if exists {
                    violations.push(Violation {
                        path: node.path.clone(),
                        message: format!("Item must not exist: {}", node.path),
                    });
                }
                continue;
            }
        }

        match node.kind {
            NodeKind::File => {
                if !full_path.is_file() {
                    violations.push(Violation {
                        path: node.path.clone(),
                        message: format!("Expected file, but found directory: {}", node.path),
                    });
                }
            }
            NodeKind::Directory => {
                if !full_path.is_dir() {
                    violations.push(Violation {
                        path: node.path.clone(),
                        message: format!("Expected directory, but found file: {}", node.path),
                    });
                }
            }
            NodeKind::Any => {
                //
            }
        }
    }

    violations
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
