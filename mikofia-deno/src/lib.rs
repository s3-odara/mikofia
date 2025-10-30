mod context;
mod ops;
mod rules;
mod runtime;

pub use context::{context_to_v8, create_context_with_fs};
pub use rules::{js_value_to_rule_result, JavaScriptRuleHandle};
pub use runtime::DenoRuntime;

use mikofia::Config;
use std::path::Path;

/// Load a JavaScript configuration file
pub async fn load_javascript_config(
    path: &Path,
) -> Result<Config, Box<dyn std::error::Error + Send + Sync>> {
    let mut runtime = DenoRuntime::new()?;
    runtime.load_config(path).await
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
}
