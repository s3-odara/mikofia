mod engine;
mod fs;
pub mod glob;
mod ignore;
mod pipeline;
mod reporter;
mod types;

// Re-export public API
pub use engine::{check, check_with_fs, check_with_fs_and_ignore, check_with_ignore};
pub use fs::{FileSystem, RealFileSystem};
pub use ignore::IgnoreMatcher;
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
            ignore: vec![],
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
            ignore: vec![],
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
            ignore: vec![],
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
                    ignore: vec![],
                    rules: vec![],
                },
                Node {
                    path: "lib.rs".to_string(),
                    existence: Existence::Required,
                    kind: NodeKind::File,
                    children: vec![],
                    strict: None,
                    ignore: vec![],
                    rules: vec![],
                },
            ],
            strict: Some(true),
            ignore: vec![],
            rules: vec![],
        }];

        let violations = check_with_fs(&nodes, &root, &mock_fs);
        if !violations.is_empty() {
            eprintln!("Violations: {:?}", violations);
        }
        assert!(violations.is_empty());
    }

    #[test]
    fn test_node_level_ignore_scoped_to_node() {
        let root = mock_root();
        let mut mock_fs = MockFileSystem::new();
        mock_fs.add_dir(root.join("apps"));
        mock_fs.add_dir(root.join("apps/web"));
        mock_fs.add_dir(root.join("apps/web/src"));
        mock_fs.add_dir(root.join("apps/web/dist"));
        mock_fs.add_file(root.join("apps/web/src/index.ts"), "export {};");

        let nodes = vec![Node {
            path: "apps".to_string(),
            existence: Existence::Required,
            kind: NodeKind::Directory,
            children: vec![Node {
                path: "web".to_string(),
                existence: Existence::Required,
                kind: NodeKind::Directory,
                children: vec![Node {
                    path: "src".to_string(),
                    existence: Existence::Required,
                    kind: NodeKind::Directory,
                    children: vec![Node {
                        path: "index.ts".to_string(),
                        existence: Existence::Required,
                        kind: NodeKind::File,
                        children: vec![],
                        strict: None,
                        ignore: vec![],
                        rules: vec![],
                    }],
                    strict: Some(true),
                    ignore: vec![],
                    rules: vec![],
                }],
                strict: Some(true),
                ignore: vec!["dist".to_string()],
                rules: vec![],
            }],
            strict: Some(true),
            ignore: vec![],
            rules: vec![],
        }];

        let violations = check_with_fs(&nodes, &root, &mock_fs);
        assert!(
            violations.is_empty(),
            "unexpected violations when node-level ignore should exclude dist: {:?}",
            violations
        );
    }

    #[test]
    fn test_node_level_ignore_with_globbed_directory() {
        let root = mock_root();
        let mut mock_fs = MockFileSystem::new();
        mock_fs.add_dir(root.join("apps"));
        mock_fs.add_dir(root.join("apps/app1"));
        mock_fs.add_dir(root.join("apps/app1/src"));
        mock_fs.add_dir(root.join("apps/app1/dist"));
        mock_fs.add_file(root.join("apps/app1/src/index.tsx"), "export {};");

        let nodes = vec![Node {
            path: "apps/*".to_string(),
            existence: Existence::Required,
            kind: NodeKind::Directory,
            children: vec![Node {
                path: "src".to_string(),
                existence: Existence::Required,
                kind: NodeKind::Directory,
                children: vec![Node {
                    path: "index.tsx".to_string(),
                    existence: Existence::Required,
                    kind: NodeKind::File,
                    children: vec![],
                    strict: None,
                    ignore: vec![],
                    rules: vec![],
                }],
                strict: Some(true),
                ignore: vec![],
                rules: vec![],
            }],
            strict: Some(true),
            ignore: vec!["dist".to_string()],
            rules: vec![],
        }];

        let violations = check_with_fs(&nodes, &root, &mock_fs);
        assert!(
            violations.is_empty(),
            "unexpected violations when globbed node-level ignore should exclude dist: {:?}",
            violations
        );
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
            ignore: vec![],
            rules: vec![],
        };
        assert!(node.is_glob_pattern());

        let node = Node {
            path: "test?.txt".to_string(),
            existence: Existence::Required,
            kind: NodeKind::File,
            children: vec![],
            strict: None,
            ignore: vec![],
            rules: vec![],
        };
        assert!(node.is_glob_pattern());

        let node = Node {
            path: "file[123].txt".to_string(),
            existence: Existence::Required,
            kind: NodeKind::File,
            children: vec![],
            strict: None,
            ignore: vec![],
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
            ignore: vec![],
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
            ignore: vec![],
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
            ignore: vec![],
            rules: vec![],
        };
        assert!(!node.is_glob_pattern());

        let node = Node {
            path: "Cargo.toml".to_string(),
            existence: Existence::Required,
            kind: NodeKind::File,
            children: vec![],
            strict: None,
            ignore: vec![],
            rules: vec![],
        };
        assert!(!node.is_glob_pattern());

        let node = Node {
            path: "path/to/file.txt".to_string(),
            existence: Existence::Required,
            kind: NodeKind::File,
            children: vec![],
            strict: None,
            ignore: vec![],
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
            ignore: vec![],
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
            ignore: vec![],
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
            ignore: vec![],
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
                    ignore: vec![],
                    rules: vec![],
                },
                Node {
                    path: "dev.toml".to_string(),
                    existence: Existence::Optional,
                    kind: NodeKind::File,
                    children: vec![],
                    strict: None,
                    ignore: vec![],
                    rules: vec![],
                },
            ],
            strict: Some(true),
            ignore: vec![],
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
                ignore: vec![],
                rules: vec![],
            }],
            strict: Some(true),
            ignore: vec![],
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
    fn test_mock_fs_strict_directory_honors_nested_ignore() {
        let root = mock_root();
        let mut mock_fs = MockFileSystem::new();
        mock_fs.add_dir(root.join("tmp"));
        mock_fs.add_dir(root.join("tmp/cache"));
        mock_fs.add_file(root.join("tmp/cache/data.txt"), "data");

        let nodes = vec![Node {
            path: "tmp".to_string(),
            existence: Existence::Required,
            kind: NodeKind::Directory,
            children: vec![],
            strict: Some(true),
            ignore: vec![],
            rules: vec![],
        }];

        let ignore = IgnoreMatcher::new(&vec!["tmp/cache".to_string()]).unwrap();

        let violations = check_with_fs_and_ignore(&nodes, &root, &mock_fs, &ignore);
        assert!(
            violations.is_empty(),
            "unexpected violations when ignoring nested path: {violations:?}"
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
                ignore: vec![],
                rules: vec![],
            }],
            strict: Some(true),
            ignore: vec![],
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
                    ignore: vec![],
                    rules: vec![],
                },
                Node {
                    path: "*.toml".to_string(),
                    existence: Existence::Optional,
                    kind: NodeKind::File,
                    children: vec![],
                    strict: None,
                    ignore: vec![],
                    rules: vec![],
                },
            ],
            strict: Some(true),
            ignore: vec![],
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
    fn test_mock_fs_strict_directory_allows_multilevel_glob_children() {
        let root = mock_root();
        let mut mock_fs = MockFileSystem::new();
        mock_fs.add_dir(root.join("src"));
        mock_fs.add_dir(root.join("src/nested"));
        mock_fs.add_dir(root.join("src/nested/deep"));
        mock_fs.add_file(
            root.join("src/nested/deep/index.ts"),
            "export const value = 1;",
        );

        let nodes = vec![Node {
            path: "src".to_string(),
            existence: Existence::Required,
            kind: NodeKind::Directory,
            children: vec![Node {
                path: "nested/**/*.ts".to_string(),
                existence: Existence::Optional,
                kind: NodeKind::File,
                children: vec![],
                strict: None,
                ignore: vec![],
                rules: vec![],
            }],
            strict: Some(true),
            ignore: vec![],
            rules: vec![],
        }];

        let violations = check_with_fs(&nodes, &root, &mock_fs);
        assert!(
            violations.is_empty(),
            "expected strict mode to allow nested/**/*.ts, violations: {violations:?}"
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
            ignore: vec![],
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
            ignore: vec![],
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
            ignore: vec![],
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
            ignore: vec![],
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
            ignore: vec![],
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
            ignore: vec![],
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
    fn test_node_level_ignore_scoped_to_node_directory() {
        let root = mock_root();
        let mut mock_fs = MockFileSystem::new();

        // Create directory structure:
        // apps/
        //   web/
        //     dist/
        //       bundle.js
        //     src/
        //       index.ts
        //   api/
        //     dist/
        //       server.js
        mock_fs.add_dir(root.join("apps"));
        mock_fs.add_dir(root.join("apps/web"));
        mock_fs.add_dir(root.join("apps/web/dist"));
        mock_fs.add_file(root.join("apps/web/dist/bundle.js"), "// bundle");
        mock_fs.add_dir(root.join("apps/web/src"));
        mock_fs.add_file(root.join("apps/web/src/index.ts"), "// source");
        mock_fs.add_dir(root.join("apps/api"));
        mock_fs.add_dir(root.join("apps/api/dist"));
        mock_fs.add_file(root.join("apps/api/dist/server.js"), "// server");

        let nodes = vec![Node {
            path: "apps".to_string(),
            existence: Existence::Required,
            kind: NodeKind::Directory,
            children: vec![
                Node {
                    path: "web".to_string(),
                    existence: Existence::Required,
                    kind: NodeKind::Directory,
                    children: vec![Node {
                        path: "src".to_string(),
                        existence: Existence::Required,
                        kind: NodeKind::Directory,
                        children: vec![],
                        strict: None,
                        ignore: vec![],
                        rules: vec![],
                    }],
                    strict: Some(true),
                    // This should ignore apps/web/dist but NOT apps/api/dist
                    ignore: vec!["dist".to_string()],
                    rules: vec![],
                },
                Node {
                    path: "api".to_string(),
                    existence: Existence::Required,
                    kind: NodeKind::Directory,
                    children: vec![],
                    strict: Some(true),
                    ignore: vec![],
                    rules: vec![],
                },
            ],
            strict: None,
            ignore: vec![],
            rules: vec![],
        }];

        let violations = check_with_fs(&nodes, &root, &mock_fs);

        // apps/web/dist should be ignored due to node-level ignore
        // apps/api/dist should be reported as unlisted-child
        assert_eq!(
            violations.len(),
            1,
            "expected one violation for apps/api/dist"
        );
        assert_eq!(violations[0].key, "unlisted-child");
        assert!(
            violations[0].path.contains("apps/api/dist"),
            "expected violation for apps/api/dist, got: {}",
            violations[0].path
        );
    }

    #[test]
    fn test_node_level_ignore_with_glob_pattern() {
        let root = mock_root();
        let mut mock_fs = MockFileSystem::new();

        // Create directory structure:
        // src/
        //   components/
        //     Button.tsx
        //     Button.test.tsx
        //     Input.tsx
        //     Input.test.tsx
        mock_fs.add_dir(root.join("src"));
        mock_fs.add_dir(root.join("src/components"));
        mock_fs.add_file(
            root.join("src/components/Button.tsx"),
            "export const Button",
        );
        mock_fs.add_file(root.join("src/components/Button.test.tsx"), "test(...)");
        mock_fs.add_file(root.join("src/components/Input.tsx"), "export const Input");
        mock_fs.add_file(root.join("src/components/Input.test.tsx"), "test(...)");

        let nodes = vec![Node {
            path: "src".to_string(),
            existence: Existence::Required,
            kind: NodeKind::Directory,
            children: vec![Node {
                path: "components".to_string(),
                existence: Existence::Required,
                kind: NodeKind::Directory,
                children: vec![Node {
                    path: "*.tsx".to_string(),
                    existence: Existence::Optional,
                    kind: NodeKind::File,
                    children: vec![],
                    strict: None,
                    ignore: vec![],
                    rules: vec![],
                }],
                strict: Some(true),
                // Ignore all test files in components directory
                ignore: vec!["*.test.tsx".to_string()],
                rules: vec![],
            }],
            strict: None,
            ignore: vec![],
            rules: vec![],
        }];

        let violations = check_with_fs(&nodes, &root, &mock_fs);

        // *.test.tsx files should be ignored, so no unlisted-child violations
        assert!(
            violations.is_empty(),
            "expected no violations when test files are ignored, got: {violations:?}"
        );
    }

    #[test]
    fn test_node_level_ignore_inherits_from_parent() {
        let root = mock_root();
        let mut mock_fs = MockFileSystem::new();

        // Create directory structure:
        // project/
        //   lib/
        //     utils.ts
        //     utils.test.ts
        //   dist/
        //     bundle.js
        mock_fs.add_dir(root.join("project"));
        mock_fs.add_dir(root.join("project/lib"));
        mock_fs.add_file(root.join("project/lib/utils.ts"), "export const utils");
        mock_fs.add_file(root.join("project/lib/utils.test.ts"), "test(...)");
        mock_fs.add_dir(root.join("project/dist"));
        mock_fs.add_file(root.join("project/dist/bundle.js"), "// bundle");

        let nodes = vec![Node {
            path: "project".to_string(),
            existence: Existence::Required,
            kind: NodeKind::Directory,
            children: vec![Node {
                path: "lib".to_string(),
                existence: Existence::Required,
                kind: NodeKind::Directory,
                children: vec![Node {
                    path: "utils.ts".to_string(),
                    existence: Existence::Required,
                    kind: NodeKind::File,
                    children: vec![],
                    strict: None,
                    ignore: vec![],
                    rules: vec![],
                }],
                strict: Some(true),
                // This should ignore *.test.ts in lib directory
                ignore: vec!["*.test.ts".to_string()],
                rules: vec![],
            }],
            strict: Some(true),
            // This should ignore dist in project directory
            ignore: vec!["dist".to_string()],
            rules: vec![],
        }];

        let violations = check_with_fs(&nodes, &root, &mock_fs);

        // Both dist and utils.test.ts should be ignored
        assert!(
            violations.is_empty(),
            "expected no violations when both parent and child ignore patterns apply, got: {violations:?}"
        );
    }

    #[test]
    fn test_nested_node_ignore_correctly_prefixed() {
        let root = mock_root();
        let mut mock_fs = MockFileSystem::new();

        // Create directory structure:
        // packages/
        //   core/
        //     lib/
        //       temp/
        //         cache.txt
        //       index.ts
        mock_fs.add_dir(root.join("packages"));
        mock_fs.add_dir(root.join("packages/core"));
        mock_fs.add_dir(root.join("packages/core/lib"));
        mock_fs.add_dir(root.join("packages/core/lib/temp"));
        mock_fs.add_file(root.join("packages/core/lib/temp/cache.txt"), "cache");
        mock_fs.add_file(root.join("packages/core/lib/index.ts"), "export *");

        let nodes = vec![Node {
            path: "packages".to_string(),
            existence: Existence::Required,
            kind: NodeKind::Directory,
            children: vec![Node {
                path: "core".to_string(),
                existence: Existence::Required,
                kind: NodeKind::Directory,
                children: vec![Node {
                    path: "lib".to_string(),
                    existence: Existence::Required,
                    kind: NodeKind::Directory,
                    children: vec![Node {
                        path: "index.ts".to_string(),
                        existence: Existence::Required,
                        kind: NodeKind::File,
                        children: vec![],
                        strict: None,
                        ignore: vec![],
                        rules: vec![],
                    }],
                    strict: Some(true),
                    // This should ignore packages/core/lib/temp
                    ignore: vec!["temp".to_string()],
                    rules: vec![],
                }],
                strict: None,
                ignore: vec![],
                rules: vec![],
            }],
            strict: None,
            ignore: vec![],
            rules: vec![],
        }];

        let violations = check_with_fs(&nodes, &root, &mock_fs);

        // packages/core/lib/temp should be ignored
        assert!(
            violations.is_empty(),
            "expected no violations when nested node ignore pattern is correctly prefixed, got: {violations:?}"
        );
    }
}
