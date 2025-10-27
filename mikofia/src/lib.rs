mod engine;
mod glob;
mod pipeline;
mod types;

// Re-export public API
pub use engine::check;
pub use types::{Config, Existence, Node, NodeKind, Violation};

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
