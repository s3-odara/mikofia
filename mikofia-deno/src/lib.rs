mod context;
mod ops;
mod rules;
mod runtime;
mod ts_loader;

pub use context::{context_to_v8, create_context_with_fs};
pub use rules::{JavaScriptRuleHandle, js_value_to_rule_result};
pub use runtime::DenoRuntime;
pub use ts_loader::TsModuleLoader;

use mikofia::Config;
use std::path::Path;

/// Load a Deno-based configuration file (JavaScript or TypeScript)
///
/// This function uses the Deno runtime to load and execute configuration files.
/// Supported formats: `.js`, `.ts`
pub async fn load_deno_config(
    path: &Path,
) -> Result<Config, Box<dyn std::error::Error + Send + Sync>> {
    let mut runtime = DenoRuntime::new()?;

    // Allow access to config file's parent directory for imports
    if let Some(parent) = path.parent() {
        runtime
            .set_allowed_roots([parent])
            .map_err(|e| format!("Failed to configure allowed paths: {}", e))?;
    }

    runtime.load_config(path).await
}

/// Load config and check with unified flow (JavaScript rules injected into config)
///
/// This is the recommended way to use mikofia with Deno configs.
/// It loads the config, extracts JavaScript rules, injects them into the config nodes,
/// and then runs the standard check flow.
///
/// # Thread Safety
///
/// This function must be called from a `tokio::task::LocalSet` because V8 requires
/// thread affinity. The JavaScript runtime and all rules will execute on the calling thread.
pub async fn load_and_check(
    config_path: &Path,
    root: &Path,
) -> Result<Vec<mikofia::Violation>, Box<dyn std::error::Error + Send + Sync>> {
    use std::cell::RefCell;
    use std::rc::Rc;

    // Create runtime
    let mut runtime = DenoRuntime::new()?;

    // Configure allowed roots: both project root AND config parent directory
    // This allows JavaScript rules to access both project files and config-relative imports
    let mut allowed_roots = vec![root.to_path_buf()];
    if let Some(config_parent) = config_path.parent() {
        let config_parent = config_parent.to_path_buf();
        // Only add if different from root
        if config_parent != root {
            allowed_roots.push(config_parent);
        }
    }
    runtime
        .set_allowed_roots(allowed_roots)
        .map_err(|e| format!("Failed to configure allowed roots: {}", e))?;

    // Load config with rules (won't override allowed_roots since we set them above)
    let (mut config, rules_map) = runtime.load_config_with_rules(config_path).await?;

    // Wrap runtime in Rc<RefCell> for local sharing (enforces single-thread access)
    let runtime_rc = Rc::new(RefCell::new(runtime));

    // Inject JavaScript rules into config nodes
    DenoRuntime::inject_rules_into_config(&mut config, rules_map, runtime_rc.clone());

    // Create ignore matcher from config
    let ignore_matcher = mikofia::IgnoreMatcher::new(&config.ignore)
        .map_err(|e| format!("Failed to create ignore matcher: {}", e))?;

    // Run unified check (includes both structure and JavaScript rules)
    let violations = mikofia::check_with_ignore(&config.nodes, root, &ignore_matcher).await;

    Ok(violations)
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

        let config = load_deno_config(&config_path).await.unwrap();
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

        let config = load_deno_config(&config_path).await.unwrap();
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

        let config = load_deno_config(&config_path).await.unwrap();
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
    async fn test_custom_rules_applied_through_unified_flow() {
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

        let violations = load_and_check(&config_path, root).await.unwrap();

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

        let violations = load_and_check(&config_path, root).await.unwrap();

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

        let violations = load_and_check(&config_path, root).await.unwrap();

        assert_eq!(violations.len(), 1);
        let violation = &violations[0];
        assert_eq!(violation.key, "js-rule-error");
        assert!(
            violation
                .message
                .contains("Direct access to Deno.core ops is disabled"),
            "unexpected violation message: {}",
            violation.message
        );
    }

    #[tokio::test]
    async fn test_load_typescript_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test.config.ts");

        let config_content = r#"
            interface Node {
                path: string;
                existence?: "required" | "optional" | "forbidden";
                kind?: "file" | "directory" | "any";
            }

            interface Config {
                nodes: Node[];
            }

            const config: Config = {
                nodes: [
                    {
                        path: "test.txt",
                        existence: "required",
                        kind: "file",
                    }
                ]
            };
            export default config;
        "#;

        fs::write(&config_path, config_content).unwrap();

        let config = load_deno_config(&config_path).await.unwrap();
        assert_eq!(config.nodes.len(), 1);
        assert_eq!(config.nodes[0].path, "test.txt");
        assert_eq!(config.nodes[0].existence, mikofia::Existence::Required);
        assert_eq!(config.nodes[0].kind, mikofia::NodeKind::File);
    }

    #[tokio::test]
    async fn test_typescript_config_with_custom_rules() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        let config_path = root.join("mikofia.config.ts");
        let config_content = r#"
            type RuleFunction = (ctx: any) => any;

            interface NodeConfig {
                path: string;
                existence?: "required" | "optional" | "forbidden";
                kind?: "file" | "directory" | "any";
                rules?: RuleFunction[];
            }

            interface Config {
                nodes: NodeConfig[];
            }

            const config: Config = {
                nodes: [
                    {
                        path: "src/**/*.ts",
                        existence: "optional",
                        kind: "file",
                        rules: [
                            (ctx) => {
                                if (ctx.name.includes("test")) {
                                    return {
                                        type: "fail",
                                        violation: {
                                            key: "no-test-in-name",
                                            message: `Test files not allowed: ${ctx.name}`
                                        }
                                    };
                                }
                                return { type: "pass" };
                            }
                        ]
                    }
                ]
            };

            export default config;
        "#;

        fs::write(&config_path, config_content).unwrap();

        let src_dir = root.join("src");
        fs::create_dir_all(&src_dir).unwrap();
        let test_file = src_dir.join("test_file.ts");
        fs::write(&test_file, "// demo").unwrap();

        let violations = load_and_check(&config_path, root).await.unwrap();

        assert_eq!(violations.len(), 1);
        let violation = &violations[0];
        assert_eq!(violation.key, "no-test-in-name");
        assert!(
            violation.message.contains("test_file.ts"),
            "violation message was {}",
            violation.message
        );
    }
}
