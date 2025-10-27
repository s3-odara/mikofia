mod engine;
mod fs;
mod glob;
mod pipeline;
mod types;

// Re-export public API
pub use engine::{check, check_with_fs};
pub use fs::{FileSystem, RealFileSystem};
pub use types::{Config, Existence, Node, NodeKind, Violation};

#[cfg(test)]
pub use fs::mock::MockFileSystem;

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

    #[test]
    fn test_mock_fs_required_file_exists() {
        let mut mock_fs = MockFileSystem::new();
        mock_fs.add_file("./test.txt", "content");

        let nodes = vec![Node {
            path: "test.txt".to_string(),
            existence: Existence::Required,
            kind: NodeKind::File,
            children: vec![],
            strict: None,
        }];

        let violations = check_with_fs(&nodes, &PathBuf::from("."), &mock_fs);
        if !violations.is_empty() {
            eprintln!("Violations: {:?}", violations);
        }
        assert!(violations.is_empty());
    }

    #[test]
    fn test_mock_fs_required_file_missing() {
        let mock_fs = MockFileSystem::new();

        let nodes = vec![Node {
            path: "missing.txt".to_string(),
            existence: Existence::Required,
            kind: NodeKind::Any,
            children: vec![],
            strict: None,
        }];

        let violations = check_with_fs(&nodes, &PathBuf::from("."), &mock_fs);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].path, "missing.txt");
    }

    #[test]
    fn test_mock_fs_directory_with_children() {
        let mut mock_fs = MockFileSystem::new();
        mock_fs.add_dir("./src");
        mock_fs.add_file("./src/main.rs", "fn main() {}");
        mock_fs.add_file("./src/lib.rs", "pub fn test() {}");

        let nodes = vec![Node {
            path: "src".to_string(),
            existence: Existence::Required,
            kind: NodeKind::Directory,
            children: vec![
                Node {
                    path: "main.rs".to_string(),
                    existence: Existence::Required,
                    kind: NodeKind::File,
                    children: vec![],
                    strict: None,
                },
                Node {
                    path: "lib.rs".to_string(),
                    existence: Existence::Required,
                    kind: NodeKind::File,
                    children: vec![],
                    strict: None,
                },
            ],
            strict: Some(true),
        }];

        let violations = check_with_fs(&nodes, &PathBuf::from("."), &mock_fs);
        if !violations.is_empty() {
            eprintln!("Violations: {:?}", violations);
        }
        assert!(violations.is_empty());
    }

    #[test]
    fn test_mock_fs_config_loading() {
        let mut mock_fs = MockFileSystem::new();
        mock_fs.add_file(
            "config.json",
            r#"{"nodes": [{"path": "test.txt", "existence": "required"}]}"#,
        );

        let config = Config::from_file_with_fs(&PathBuf::from("config.json"), &mock_fs)
            .expect("Failed to load config");

        assert_eq!(config.nodes.len(), 1);
        assert_eq!(config.nodes[0].path, "test.txt");
        assert_eq!(config.nodes[0].existence, Existence::Required);
    }
}
