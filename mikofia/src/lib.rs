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

    #[test]
    fn test_is_glob_pattern_basic_wildcards() {
        // Basic glob patterns
        let node = Node {
            path: "src/*.rs".to_string(),
            existence: Existence::Required,
            kind: NodeKind::File,
            children: vec![],
            strict: None,
        };
        assert!(node.is_glob_pattern());

        let node = Node {
            path: "test?.txt".to_string(),
            existence: Existence::Required,
            kind: NodeKind::File,
            children: vec![],
            strict: None,
        };
        assert!(node.is_glob_pattern());

        let node = Node {
            path: "file[123].txt".to_string(),
            existence: Existence::Required,
            kind: NodeKind::File,
            children: vec![],
            strict: None,
        };
        assert!(node.is_glob_pattern());
    }

    #[test]
    fn test_is_glob_pattern_brace_expansion() {
        // Brace expansion pattern
        let node = Node {
            path: "src/{foo,bar}.rs".to_string(),
            existence: Existence::Required,
            kind: NodeKind::File,
            children: vec![],
            strict: None,
        };
        assert!(node.is_glob_pattern());
    }

    #[test]
    fn test_is_glob_pattern_negation() {
        // Negation pattern
        let node = Node {
            path: "!*.tmp".to_string(),
            existence: Existence::Absent,
            kind: NodeKind::File,
            children: vec![],
            strict: None,
        };
        assert!(node.is_glob_pattern());
    }

    #[test]
    fn test_is_glob_pattern_literal_paths() {
        // Literal paths should not be detected as globs
        let node = Node {
            path: "src/main.rs".to_string(),
            existence: Existence::Required,
            kind: NodeKind::File,
            children: vec![],
            strict: None,
        };
        assert!(!node.is_glob_pattern());

        let node = Node {
            path: "Cargo.toml".to_string(),
            existence: Existence::Required,
            kind: NodeKind::File,
            children: vec![],
            strict: None,
        };
        assert!(!node.is_glob_pattern());

        let node = Node {
            path: "path/to/file.txt".to_string(),
            existence: Existence::Required,
            kind: NodeKind::File,
            children: vec![],
            strict: None,
        };
        assert!(!node.is_glob_pattern());
    }

    #[test]
    fn test_mock_fs_absent_violation() {
        let mut mock_fs = MockFileSystem::new();
        mock_fs.add_file("./temp.log", "data");

        let nodes = vec![Node {
            path: "temp.log".to_string(),
            existence: Existence::Absent,
            kind: NodeKind::Any,
            children: vec![],
            strict: None,
        }];

        let violations = check_with_fs(&nodes, &PathBuf::from("."), &mock_fs);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].message, "Item must not exist: temp.log");
    }

    #[test]
    fn test_mock_fs_expected_directory_but_found_file() {
        let mut mock_fs = MockFileSystem::new();
        mock_fs.add_file("./config", "{}");

        let nodes = vec![Node {
            path: "config".to_string(),
            existence: Existence::Required,
            kind: NodeKind::Directory,
            children: vec![],
            strict: None,
        }];

        let violations = check_with_fs(&nodes, &PathBuf::from("."), &mock_fs);
        assert_eq!(violations.len(), 1);
        assert_eq!(
            violations[0].message,
            "Expected directory, but found file: config"
        );
    }

    #[test]
    fn test_mock_fs_expected_file_but_found_directory() {
        let mut mock_fs = MockFileSystem::new();
        mock_fs.add_dir("./src");

        let nodes = vec![Node {
            path: "src".to_string(),
            existence: Existence::Required,
            kind: NodeKind::File,
            children: vec![],
            strict: None,
        }];

        let violations = check_with_fs(&nodes, &PathBuf::from("."), &mock_fs);
        assert_eq!(violations.len(), 1);
        assert_eq!(
            violations[0].message,
            "Expected file, but found directory: src"
        );
    }
}
