mod context;
mod ops;
mod rules;
mod runtime;

pub use context::{context_to_v8, create_context_with_fs};
pub use rules::{JavaScriptRuleHandle, js_value_to_rule_result};
pub use runtime::DenoRuntime;

use mikofia::Config;
use std::path::{Component, Path, PathBuf};

/// Load a JavaScript configuration file
pub async fn load_javascript_config(
    path: &Path,
) -> Result<Config, Box<dyn std::error::Error + Send + Sync>> {
    let mut runtime = DenoRuntime::new()?;
    runtime.load_config(path).await
}

/// Check with JavaScript custom rules
pub async fn check_with_javascript_rules(
    config: Config,
    root: &Path,
    rules_map: Vec<(String, Vec<JavaScriptRuleHandle>)>,
    runtime: &mut DenoRuntime,
) -> Result<Vec<mikofia::Violation>, Box<dyn std::error::Error + Send + Sync>> {
    use mikofia::{IgnoreMatcher, RealFileSystem};

    runtime
        .set_allowed_roots([root])
        .map_err(|e| format!("Failed to configure project root: {}", e))?;

    // Create ignore matcher from config
    let ignore_matcher = IgnoreMatcher::new(&config.ignore)
        .map_err(|e| format!("Failed to create ignore matcher: {}", e))?;

    // 1. Run standard checks with ignore patterns
    let mut violations = mikofia::check_with_ignore(&config.nodes, root, &ignore_matcher);

    // 2. Run JavaScript custom rules
    let fs = RealFileSystem;

    for (pattern, rules) in rules_map {
        // Find matching files for this pattern
        let matched_paths = find_matching_paths(root, &pattern, &fs)?;

        // Filter out ignored paths
        let filtered_paths: Vec<PathBuf> = matched_paths
            .into_iter()
            .filter(|path| {
                path.strip_prefix(root)
                    .ok()
                    .map(|relative| !ignore_matcher.is_ignored(relative))
                    .unwrap_or(true)
            })
            .collect();

        for path in filtered_paths {
            // Build evaluation context
            let ctx = build_evaluation_context(&path, &fs)?;

            // Execute each rule
            for rule in &rules {
                match runtime.call_rule(rule.function(), &ctx).await {
                    Ok(mikofia::RuleResult::Fail { mut violation }) => {
                        // Set the path if not already set by the rule
                        if violation.path.is_empty() {
                            violation.path = path.to_string_lossy().to_string();
                        }
                        violations.push(violation);
                    }
                    Ok(mikofia::RuleResult::Pass) => {}
                    Ok(mikofia::RuleResult::Skip { .. }) => {}
                    Err(e) => {
                        violations.push(mikofia::Violation::new(
                            "rule-execution-error",
                            path.to_string_lossy().to_string(),
                            format!("Failed to execute rule: {}", e),
                        ));
                    }
                }
            }
        }
    }

    Ok(violations)
}

/// Find files matching a pattern
fn find_matching_paths(
    root: &Path,
    pattern: &str,
    fs: &impl mikofia::FileSystem,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error + Send + Sync>> {
    // If pattern contains glob syntax, use glob expansion
    if mikofia::glob::is_glob_pattern(pattern) {
        if let Some((literal_prefix, remainder_pattern)) = split_literal_prefix(pattern) {
            let base_dir = root.join(&literal_prefix);

            if !fs.exists(&base_dir) || !fs.is_dir(&base_dir) {
                return Ok(vec![]);
            }

            return match mikofia::glob::expand_glob(&remainder_pattern, &base_dir, fs) {
                Ok(paths) => Ok(paths),
                Err(_) => Ok(vec![]),
            };
        }

        // Use mikofia's glob expansion which is already optimized
        match mikofia::glob::expand_glob(pattern, root, fs) {
            Ok(paths) => Ok(paths),
            Err(_) => Ok(vec![]), // Return empty if glob expansion fails
        }
    } else {
        // Literal path
        let full_path = root.join(pattern);
        if fs.exists(&full_path) {
            Ok(vec![full_path])
        } else {
            Ok(vec![])
        }
    }
}

fn split_literal_prefix(pattern: &str) -> Option<(PathBuf, String)> {
    use globset::{GlobBuilder, escape};

    let mut literal_prefix = PathBuf::new();
    let mut remainder: Vec<String> = Vec::new();
    let mut glob_found = false;

    for component in Path::new(pattern).components() {
        let component_str = match component {
            Component::Normal(segment) => segment.to_string_lossy().into_owned(),
            Component::CurDir => ".".to_string(),
            Component::ParentDir => "..".to_string(),
            _ => return None,
        };

        let is_glob = GlobBuilder::new(&component_str)
            .literal_separator(true)
            .build()
            .map(|parsed| {
                let escaped = escape(&component_str);
                GlobBuilder::new(&escaped)
                    .literal_separator(true)
                    .build()
                    .map(|literal| parsed.regex() != literal.regex())
                    .unwrap_or(true)
            })
            .unwrap_or(true);

        if !glob_found && !is_glob {
            literal_prefix.push(&component_str);
        } else {
            glob_found = true;
            remainder.push(component_str);
        }
    }

    if literal_prefix.as_os_str().is_empty() || remainder.is_empty() {
        None
    } else {
        Some((literal_prefix, remainder.join("/")))
    }
}

/// Build evaluation context for a path
fn build_evaluation_context(
    path: &Path,
    fs: &impl mikofia::FileSystem,
) -> Result<mikofia::EvaluationContext, Box<dyn std::error::Error + Send + Sync>> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_string());

    let parent = path.parent().and_then(|parent_path| {
        parent_path.file_name().and_then(|parent_name| {
            parent_name.to_str().map(|name_str| mikofia::ParentInfo {
                path: parent_path.to_string_lossy().to_string(),
                name: name_str.to_string(),
            })
        })
    });

    let siblings = if let Some(parent_path) = path.parent() {
        fs.read_dir(parent_path)
            .unwrap_or_default()
            .into_iter()
            .filter(|sibling_name| sibling_name != &name)
            .map(|sibling_name| {
                let sibling_path = parent_path.join(&sibling_name);
                mikofia::SiblingInfo {
                    name: sibling_name,
                    is_file: !fs.is_dir(&sibling_path),
                    is_directory: fs.is_dir(&sibling_path),
                }
            })
            .collect()
    } else {
        vec![]
    };

    Ok(mikofia::EvaluationContext {
        path: path.to_string_lossy().to_string(),
        name,
        extension,
        parent,
        siblings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_load_simple_javascript_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test.config.js");

        let config_content = r#"
            const config = {
                nodes: [
                    {
                        path: "test.txt",
                        existence: "required",
                        kind: "file",
                        children: [],
                        strict: null,
                        rules: [],
                    }
                ]
            };
            export default config;
        "#;

        fs::write(&config_path, config_content).unwrap();

        let config = load_javascript_config(&config_path).await.unwrap();
        assert_eq!(config.nodes.len(), 1);
        assert_eq!(config.nodes[0].path, "test.txt");
        assert_eq!(config.nodes[0].existence, mikofia::Existence::Required);
        assert_eq!(config.nodes[0].kind, mikofia::NodeKind::File);
    }

    #[tokio::test]
    async fn test_load_config_with_defaults() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test2.config.js");

        let config_content = r#"
            export default {
                nodes: [
                    {
                        path: "src",
                    }
                ]
            };
        "#;

        fs::write(&config_path, config_content).unwrap();

        let config = load_javascript_config(&config_path).await.unwrap();
        assert_eq!(config.nodes.len(), 1);
        assert_eq!(config.nodes[0].path, "src");
        assert_eq!(config.nodes[0].existence, mikofia::Existence::Optional);
        assert_eq!(config.nodes[0].kind, mikofia::NodeKind::Any);
    }

    #[tokio::test]
    async fn test_load_config_with_children() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test3.config.js");

        let config_content = r#"
            export default {
                nodes: [
                    {
                        path: "src",
                        existence: "required",
                        kind: "directory",
                        children: [
                            {
                                path: "main.rs",
                                existence: "required",
                                kind: "file",
                            },
                            {
                                path: "lib.rs",
                                existence: "optional",
                                kind: "file",
                            }
                        ],
                        strict: true,
                    }
                ]
            };
        "#;

        fs::write(&config_path, config_content).unwrap();

        let config = load_javascript_config(&config_path).await.unwrap();
        assert_eq!(config.nodes.len(), 1);
        assert_eq!(config.nodes[0].children.len(), 2);
        assert_eq!(config.nodes[0].children[0].path, "main.rs");
        assert_eq!(config.nodes[0].children[1].path, "lib.rs");
        assert_eq!(config.nodes[0].strict, Some(true));
    }

    #[tokio::test]
    async fn test_load_config_with_rules_extracts_functions() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("rules.config.js");

        let config_content = r#"
            export default {
                nodes: [
                    {
                        path: "src/**/*.ts",
                        rules: [
                            async () => ({ type: "pass" }),
                            async () => ({ type: "skip", reason: "noop" })
                        ],
                        children: [
                            {
                                path: "nested/*.js",
                                rules: [
                                    () => ({
                                        type: "fail",
                                        violation: {
                                            key: "demo",
                                            message: "should not see this message"
                                        }
                                    })
                                ]
                            }
                        ]
                    }
                ]
            };
        "#;

        fs::write(&config_path, config_content).unwrap();

        let mut runtime = DenoRuntime::new().unwrap();
        let (config, rules_map) = runtime.load_config_with_rules(&config_path).await.unwrap();

        assert_eq!(config.nodes.len(), 1);

        let mut pattern_counts: Vec<(String, usize)> = rules_map
            .iter()
            .map(|(pattern, rules)| (pattern.clone(), rules.len()))
            .collect();
        pattern_counts.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(
            pattern_counts,
            vec![
                ("src/**/*.ts".to_string(), 2),
                ("src/**/*.ts/nested/*.js".to_string(), 1)
            ]
        );
    }

    #[tokio::test]
    async fn test_check_with_javascript_rules_applies_custom_rule() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        let config_path = root.join("mikofia.config.js");
        let config_content = r#"
            export default {
                nodes: [
                    {
                        path: "src/**/*.ts",
                        existence: "optional",
                        kind: "file",
                        rules: [
                            (ctx) => {
                                if (ctx.name.includes("bad")) {
                                    return {
                                        type: "fail",
                                        violation: {
                                            key: "invalid-name",
                                            message: `Unexpected filename: ${ctx.name}`
                                        }
                                    };
                                }
                                return { type: "pass" };
                            }
                        ]
                    }
                ]
            };
        "#;

        fs::write(&config_path, config_content).unwrap();

        let src_dir = root.join("src/components");
        fs::create_dir_all(&src_dir).unwrap();
        let bad_file = src_dir.join("bad_file.ts");
        fs::write(&bad_file, "// demo").unwrap();

        let mut runtime = DenoRuntime::new().unwrap();
        let (config, rules_map) = runtime.load_config_with_rules(&config_path).await.unwrap();

        let violations = check_with_javascript_rules(config, root, rules_map, &mut runtime)
            .await
            .unwrap();

        assert_eq!(violations.len(), 1);
        let violation = &violations[0];
        assert_eq!(violation.key, "invalid-name");
        assert!(
            violation.message.contains("bad_file.ts"),
            "violation message was {}",
            violation.message
        );
        assert!(
            violation.path.ends_with("bad_file.ts"),
            "violation path was {}",
            violation.path
        );
    }

    #[tokio::test]
    async fn test_ctx_fs_read_file_available() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        let config_path = root.join("mikofia.config.js");
        let config_content = r#"
            export default {
                nodes: [
                    {
                        path: "src/file.txt",
                        existence: "required",
                        kind: "file",
                        rules: [
                            async (ctx) => {
                                const content = await ctx.fs.readFile(ctx.path);
                                if (content.trim() !== "hello") {
                                    return {
                                        type: "fail",
                                        violation: {
                                            key: "unexpected-content",
                                            message: `Unexpected content: ${content}`
                                        }
                                    };
                                }
                                return { type: "pass" };
                            }
                        ]
                    }
                ]
            };
        "#;

        fs::write(&config_path, config_content).unwrap();

        let src_dir = root.join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("file.txt"), "hello\n").unwrap();

        let mut runtime = DenoRuntime::new().unwrap();
        let (config, rules_map) = runtime.load_config_with_rules(&config_path).await.unwrap();

        let violations = check_with_javascript_rules(config, root, rules_map, &mut runtime)
            .await
            .unwrap();

        assert!(
            violations.is_empty(),
            "expected ctx.fs.readFile to succeed, got violations: {:?}",
            violations
        );
    }

    #[tokio::test]
    async fn test_direct_deno_core_ops_access_denied() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        let config_path = root.join("mikofia.config.js");
        let config_content = r#"
            export default {
                nodes: [
                    {
                        path: "src/file.txt",
                        existence: "required",
                        kind: "file",
                        rules: [
                            (ctx) => {
                                Deno.core.ops.op_read_file(ctx.path);
                                return { type: "pass" };
                            }
                        ]
                    }
                ]
            };
        "#;

        fs::write(&config_path, config_content).unwrap();

        let src_dir = root.join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("file.txt"), "hello\n").unwrap();

        let mut runtime = DenoRuntime::new().unwrap();
        let (config, rules_map) = runtime.load_config_with_rules(&config_path).await.unwrap();

        let violations = check_with_javascript_rules(config, root, rules_map, &mut runtime)
            .await
            .unwrap();

        assert_eq!(violations.len(), 1);
        let violation = &violations[0];
        assert_eq!(violation.key, "rule-execution-error");
        assert!(
            violation
                .message
                .contains("Direct access to Deno.core ops is disabled"),
            "unexpected violation message: {}",
            violation.message
        );
    }
}
