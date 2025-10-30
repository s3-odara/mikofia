mod engine;
mod fs;
pub mod glob;
mod pipeline;
mod reporter;
mod types;

// Re-export public API
pub use engine::{check, check_with_fs};
pub use fs::{FileSystem, RealFileSystem};
pub use reporter::{ConsoleReporter, EvaluationResult, Reporter, violations_to_results};
pub use types::{
    Config, EvaluationContext, Existence, NativeRule, Node, NodeKind, ParentInfo, RuleHandle,
    RuleResult, SiblingInfo, Violation,
};

#[cfg(test)]
pub use fs::mock::MockFileSystem;

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::io;
    use std::path::{Path, PathBuf};

    fn mock_root() -> PathBuf {
        PathBuf::from("/project")
    }

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
            rules: vec![],
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
        let root = mock_root();
        let mut mock_fs = MockFileSystem::new();
        mock_fs.add_file(root.join("test.txt"), "content");

        let nodes = vec![Node {
            path: "test.txt".to_string(),
            existence: Existence::Required,
            kind: NodeKind::File,
            children: vec![],
            strict: None,
            rules: vec![],
        }];

        let violations = check_with_fs(&nodes, &root, &mock_fs);
        if !violations.is_empty() {
            eprintln!("Violations: {:?}", violations);
        }
        assert!(violations.is_empty());
    }

    #[test]
    fn test_mock_fs_required_file_missing() {
        let mock_fs = MockFileSystem::new();
        let root = mock_root();

        let nodes = vec![Node {
            path: "missing.txt".to_string(),
            existence: Existence::Required,
            kind: NodeKind::Any,
            children: vec![],
            strict: None,
            rules: vec![],
        }];

        let violations = check_with_fs(&nodes, &root, &mock_fs);
        assert_eq!(violations.len(), 1);
        assert_eq!(
            violations[0].path,
            root.join("missing.txt").to_string_lossy().into_owned()
        );
    }

    #[test]
    fn test_mock_fs_directory_with_children() {
        let root = mock_root();
        let mut mock_fs = MockFileSystem::new();
        mock_fs.add_dir(root.join("src"));
        mock_fs.add_file(root.join("src/main.rs"), "fn main() {}");
        mock_fs.add_file(root.join("src/lib.rs"), "pub fn test() {}");

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
                    rules: vec![],
                },
                Node {
                    path: "lib.rs".to_string(),
                    existence: Existence::Required,
                    kind: NodeKind::File,
                    children: vec![],
                    strict: None,
                    rules: vec![],
                },
            ],
            strict: Some(true),
            rules: vec![],
        }];

        let violations = check_with_fs(&nodes, &root, &mock_fs);
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
            rules: vec![],
        };
        assert!(node.is_glob_pattern());

        let node = Node {
            path: "test?.txt".to_string(),
            existence: Existence::Required,
            kind: NodeKind::File,
            children: vec![],
            strict: None,
            rules: vec![],
        };
        assert!(node.is_glob_pattern());

        let node = Node {
            path: "file[123].txt".to_string(),
            existence: Existence::Required,
            kind: NodeKind::File,
            children: vec![],
            strict: None,
            rules: vec![],
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
            rules: vec![],
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
            rules: vec![],
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
            rules: vec![],
        };
        assert!(!node.is_glob_pattern());

        let node = Node {
            path: "Cargo.toml".to_string(),
            existence: Existence::Required,
            kind: NodeKind::File,
            children: vec![],
            strict: None,
            rules: vec![],
        };
        assert!(!node.is_glob_pattern());

        let node = Node {
            path: "path/to/file.txt".to_string(),
            existence: Existence::Required,
            kind: NodeKind::File,
            children: vec![],
            strict: None,
            rules: vec![],
        };
        assert!(!node.is_glob_pattern());
    }

    #[test]
    fn test_mock_fs_absent_violation() {
        let root = mock_root();
        let mut mock_fs = MockFileSystem::new();
        mock_fs.add_file(root.join("temp.log"), "data");

        let nodes = vec![Node {
            path: "temp.log".to_string(),
            existence: Existence::Absent,
            kind: NodeKind::Any,
            children: vec![],
            strict: None,
            rules: vec![],
        }];

        let violations = check_with_fs(&nodes, &root, &mock_fs);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].key, "item-must-not-exist");
        assert_eq!(violations[0].message, "Item must not exist: temp.log");
    }

    #[test]
    fn test_mock_fs_expected_directory_but_found_file() {
        let root = mock_root();
        let mut mock_fs = MockFileSystem::new();
        mock_fs.add_file(root.join("config"), "{}");

        let nodes = vec![Node {
            path: "config".to_string(),
            existence: Existence::Required,
            kind: NodeKind::Directory,
            children: vec![],
            strict: None,
            rules: vec![],
        }];

        let violations = check_with_fs(&nodes, &root, &mock_fs);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].key, "expected-directory-found-file");
        assert_eq!(
            violations[0].message,
            "Expected directory, but found file: config"
        );
    }

    #[test]
    fn test_mock_fs_expected_file_but_found_directory() {
        let root = mock_root();
        let mut mock_fs = MockFileSystem::new();
        mock_fs.add_dir(root.join("src"));

        let nodes = vec![Node {
            path: "src".to_string(),
            existence: Existence::Required,
            kind: NodeKind::File,
            children: vec![],
            strict: None,
            rules: vec![],
        }];

        let violations = check_with_fs(&nodes, &root, &mock_fs);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].key, "expected-file-found-directory");
        assert_eq!(
            violations[0].message,
            "Expected file, but found directory: src"
        );
    }

    #[test]
    fn test_mock_fs_strict_directory_allows_listed_children() {
        let root = mock_root();
        let mut mock_fs = MockFileSystem::new();
        mock_fs.add_dir(root.join("config"));
        mock_fs.add_file(root.join("config/app.toml"), "[settings]");
        mock_fs.add_file(root.join("config/dev.toml"), "[dev]");

        let nodes = vec![Node {
            path: "config".to_string(),
            existence: Existence::Required,
            kind: NodeKind::Directory,
            children: vec![
                Node {
                    path: "app.toml".to_string(),
                    existence: Existence::Required,
                    kind: NodeKind::File,
                    children: vec![],
                    strict: None,
                    rules: vec![],
                },
                Node {
                    path: "dev.toml".to_string(),
                    existence: Existence::Optional,
                    kind: NodeKind::File,
                    children: vec![],
                    strict: None,
                    rules: vec![],
                },
            ],
            strict: Some(true),
            rules: vec![],
        }];

        let violations = check_with_fs(&nodes, &root, &mock_fs);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_mock_fs_strict_directory_reports_unlisted_child() {
        let root = mock_root();
        let mut mock_fs = MockFileSystem::new();
        mock_fs.add_dir(root.join("config"));
        mock_fs.add_file(root.join("config/app.toml"), "[settings]");
        mock_fs.add_file(root.join("config/secret.toml"), "[secret]");

        let nodes = vec![Node {
            path: "config".to_string(),
            existence: Existence::Required,
            kind: NodeKind::Directory,
            children: vec![Node {
                path: "app.toml".to_string(),
                existence: Existence::Required,
                kind: NodeKind::File,
                children: vec![],
                strict: None,
                rules: vec![],
            }],
            strict: Some(true),
            rules: vec![],
        }];

        let violations = check_with_fs(&nodes, &root, &mock_fs);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].key, "unlisted-child");
        assert_eq!(violations[0].message, "Unlisted child item: secret.toml");
        assert_eq!(
            violations[0].path,
            root.join("config/secret.toml")
                .to_string_lossy()
                .into_owned()
        );
    }

    #[test]
    fn test_mock_fs_strict_directory_literal_with_relative_prefix() {
        let root = mock_root();
        let mut mock_fs = MockFileSystem::new();
        mock_fs.add_dir(root.join("config"));
        mock_fs.add_file(root.join("config/app.toml"), "[settings]");

        let nodes = vec![Node {
            path: "config".to_string(),
            existence: Existence::Required,
            kind: NodeKind::Directory,
            children: vec![Node {
                path: "./app.toml".to_string(),
                existence: Existence::Required,
                kind: NodeKind::File,
                children: vec![],
                strict: None,
                rules: vec![],
            }],
            strict: Some(true),
            rules: vec![],
        }];

        assert!(
            !nodes[0].children[0].is_glob_pattern(),
            "./app.toml should be treated as literal for this test"
        );

        let violations = check_with_fs(&nodes, &root, &mock_fs);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_mock_fs_strict_directory_mixed_literal_and_glob() {
        let root = mock_root();
        let mut mock_fs = MockFileSystem::new();
        mock_fs.add_dir(root.join("config"));
        mock_fs.add_file(root.join("config/app.toml"), "[settings]");
        mock_fs.add_file(root.join("config/dev.toml"), "[dev]");
        mock_fs.add_file(root.join("config/app.yaml"), "foo: bar");

        let nodes = vec![Node {
            path: "config".to_string(),
            existence: Existence::Required,
            kind: NodeKind::Directory,
            children: vec![
                Node {
                    path: "app.toml".to_string(),
                    existence: Existence::Required,
                    kind: NodeKind::File,
                    children: vec![],
                    strict: None,
                    rules: vec![],
                },
                Node {
                    path: "*.toml".to_string(),
                    existence: Existence::Optional,
                    kind: NodeKind::File,
                    children: vec![],
                    strict: None,
                    rules: vec![],
                },
            ],
            strict: Some(true),
            rules: vec![],
        }];

        let violations = check_with_fs(&nodes, &root, &mock_fs);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].key, "unlisted-child");
        assert_eq!(violations[0].message, "Unlisted child item: app.yaml");
        assert_eq!(
            violations[0].path,
            root.join("config/app.yaml").to_string_lossy().into_owned()
        );
    }

    #[test]
    fn test_mock_fs_glob_violation_uses_absolute_path() {
        let root = mock_root();
        let mut mock_fs = MockFileSystem::new();
        mock_fs.add_dir(root.join("config"));
        mock_fs.add_file(root.join("config/app.toml"), "[settings]");

        let nodes = vec![Node {
            path: "config/*.toml".to_string(),
            existence: Existence::Required,
            kind: NodeKind::Directory,
            children: vec![],
            strict: None,
            rules: vec![],
        }];

        let violations = check_with_fs(&nodes, &root, &mock_fs);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].key, "expected-directory-found-file");
        assert_eq!(
            violations[0].path,
            root.join("config/app.toml").to_string_lossy().into_owned()
        );
        assert_eq!(
            violations[0].message,
            "Expected directory, but found file: config/*.toml"
        );
    }

    #[test]
    fn test_mock_fs_glob_missing_uses_absolute_pattern() {
        let root = mock_root();
        let mock_fs = MockFileSystem::new();

        let nodes = vec![Node {
            path: "logs/*.log".to_string(),
            existence: Existence::Required,
            kind: NodeKind::File,
            children: vec![],
            strict: None,
            rules: vec![],
        }];

        let violations = check_with_fs(&nodes, &root, &mock_fs);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].key, "no-files-match-pattern");
        assert_eq!(
            violations[0].path,
            root.join("logs/*.log").to_string_lossy().into_owned()
        );
        assert_eq!(
            violations[0].message,
            "No files match required pattern: logs/*.log"
        );
    }

    #[test]
    fn test_optional_glob_missing_produces_no_violation() {
        let root = mock_root();
        let mock_fs = MockFileSystem::new();

        let nodes = vec![Node {
            path: "logs/*.log".to_string(),
            existence: Existence::Optional,
            kind: NodeKind::File,
            children: vec![],
            strict: None,
            rules: vec![],
        }];

        let violations = check_with_fs(&nodes, &root, &mock_fs);
        assert!(violations.is_empty());
    }

    struct ReadDirErrorFs {
        root: PathBuf,
        read_dir_called: Cell<bool>,
    }

    impl ReadDirErrorFs {
        fn new(root: PathBuf) -> Self {
            Self {
                root,
                read_dir_called: Cell::new(false),
            }
        }
    }

    impl FileSystem for ReadDirErrorFs {
        fn exists(&self, path: &Path) -> bool {
            path == self.root || path == self.root.join("config")
        }

        fn is_dir(&self, path: &Path) -> bool {
            path == self.root.join("config")
        }

        fn read_dir(&self, path: &Path) -> io::Result<Vec<String>> {
            if path == self.root.join("config") {
                self.read_dir_called.set(true);
                Err(io::Error::new(
                    io::ErrorKind::Other,
                    "simulated read_dir failure",
                ))
            } else {
                Err(io::Error::new(io::ErrorKind::NotFound, "not found"))
            }
        }

        fn read_to_string(&self, _path: &Path) -> io::Result<String> {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "read_to_string not supported",
            ))
        }

        fn walk_dir<F>(&self, _path: &Path, _predicate: F) -> io::Result<Vec<PathBuf>>
        where
            F: Fn(&Path) -> bool,
        {
            Ok(vec![])
        }
    }

    #[test]
    fn test_strict_directory_ignores_read_dir_errors() {
        let root = mock_root();
        let fs = ReadDirErrorFs::new(root.clone());

        let nodes = vec![Node {
            path: "config".to_string(),
            existence: Existence::Required,
            kind: NodeKind::Directory,
            children: vec![],
            strict: Some(true),
            rules: vec![],
        }];

        let violations = check_with_fs(&nodes, &root, &fs);
        assert!(
            violations.is_empty(),
            "unexpected violations: {violations:?}"
        );
        assert!(fs.read_dir_called.get());
    }

    #[test]
    fn test_permission_denied_directory_reports_violation() {
        let root = mock_root();
        let mut mock_fs = MockFileSystem::new();
        let protected = root.join("protected");

        mock_fs.add_dir(protected.clone());
        mock_fs.deny_dir(protected.clone());

        let nodes = vec![Node {
            path: "protected".to_string(),
            existence: Existence::Required,
            kind: NodeKind::Directory,
            children: vec![],
            strict: Some(true),
            rules: vec![],
        }];

        let violations = check_with_fs(&nodes, &root, &mock_fs);
        assert_eq!(violations.len(), 1, "expected single violation");
        let violation = &violations[0];
        assert_eq!(violation.key, "permission-denied");
        assert!(
            violation.message.contains("insufficient permissions"),
            "unexpected message: {}",
            violation.message
        );
    }

    #[test]
    fn test_permission_denied_glob_reports_violation() {
        let root = mock_root();
        let mut mock_fs = MockFileSystem::new();
        let protected = root.join("logs");

        mock_fs.add_dir(protected.clone());
        mock_fs.deny_dir(protected.clone());

        let nodes = vec![Node {
            path: "logs/**/*.log".to_string(),
            existence: Existence::Optional,
            kind: NodeKind::Any,
            children: vec![],
            strict: None,
            rules: vec![],
        }];

        let violations = check_with_fs(&nodes, &root, &mock_fs);
        assert_eq!(violations.len(), 1, "expected single violation");
        let violation = &violations[0];
        assert_eq!(violation.key, "permission-denied");
        assert!(
            violation.message.contains("insufficient permissions"),
            "unexpected message: {}",
            violation.message
        );
    }
}
