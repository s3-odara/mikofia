use std::fs;
use tempfile::TempDir;

/// Test that check_from_config_path works with JSON config
#[tokio::test(flavor = "current_thread")]
async fn test_unified_interface_with_json_config() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    let config = r#"{
        "nodes": [
            {
                "path": "src/main.rs",
                "existence": "required",
                "kind": "file"
            }
        ]
    }"#;

    fs::write(root.join("mikofia.config.json"), config).unwrap();

    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("main.rs"), "fn main() {}").unwrap();

    let config_path = root.join("mikofia.config.json");

    let violations = mikofia_deno::check_from_config_path(&config_path, root)
        .await
        .unwrap();

    assert!(
        violations.is_empty(),
        "Expected no violations, got: {:?}",
        violations
    );
}

/// Test that check_from_config_path works with Deno config
#[tokio::test(flavor = "current_thread")]
async fn test_unified_interface_with_deno_config() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    let config = r#"
        export default {
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
    "#;

    fs::write(root.join("mikofia.config.js"), config).unwrap();

    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("test_file.ts"), "// demo").unwrap();

    let config_path = root.join("mikofia.config.js");

    let violations = mikofia_deno::check_from_config_path(&config_path, root)
        .await
        .unwrap();

    assert_eq!(violations.len(), 1);
    let violation = &violations[0];
    assert_eq!(violation.key, "no-test-in-name");
    assert!(violation.message.contains("test_file.ts"));
}

/// Test that multiple JavaScript rules are applied in order
#[tokio::test(flavor = "current_thread")]
async fn test_multiple_js_rules_applied() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    let config = r#"
        export default {
            nodes: [
                {
                    path: "src/**/*.ts",
                    existence: "optional",
                    kind: "file",
                    rules: [
                        (ctx) => {
                            const basename = ctx.name.replace(/\.ts$/, "");
                            if (basename.length < 3) {
                                return {
                                    type: "fail",
                                    violation: {
                                        key: "name-too-short",
                                        message: "Filename too short"
                                    }
                                };
                            }
                            return { type: "pass" };
                        },
                        (ctx) => {
                            const basename = ctx.name.replace(/\.ts$/, "");
                            if (basename.toUpperCase() === basename && basename.length > 0) {
                                return {
                                    type: "fail",
                                    violation: {
                                        key: "all-uppercase",
                                        message: "Filename is all uppercase"
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

    fs::write(root.join("mikofia.config.js"), config).unwrap();

    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("AB.ts"), "// demo").unwrap();

    let config_path = root.join("mikofia.config.js");

    let violations = mikofia_deno::check_from_config_path(&config_path, root)
        .await
        .unwrap();

    // Should have 2 violations: one for short name, one for uppercase
    assert_eq!(violations.len(), 2);
    assert!(violations.iter().any(|v| v.key == "name-too-short"));
    assert!(violations.iter().any(|v| v.key == "all-uppercase"));
}

/// Test that JavaScript rules with async operations work
#[tokio::test(flavor = "current_thread")]
async fn test_js_rules_with_async_operations() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    let config = r#"
        export default {
            nodes: [
                {
                    path: "src/**/*.txt",
                    existence: "optional",
                    kind: "file",
                    rules: [
                        async (ctx) => {
                            const content = await ctx.fs.readFile(ctx.path);
                            if (content.includes("forbidden")) {
                                return {
                                    type: "fail",
                                    violation: {
                                        key: "forbidden-content",
                                        message: "File contains forbidden keyword"
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

    fs::write(root.join("mikofia.config.js"), config).unwrap();

    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("good.txt"), "hello world").unwrap();
    fs::write(src_dir.join("bad.txt"), "this is forbidden").unwrap();

    let config_path = root.join("mikofia.config.js");

    let violations = mikofia_deno::check_from_config_path(&config_path, root)
        .await
        .unwrap();

    assert_eq!(violations.len(), 1);
    let violation = &violations[0];
    assert_eq!(violation.key, "forbidden-content");
    assert!(violation.path.ends_with("bad.txt"));
}

/// Test that JSON and Deno configs produce same results for equivalent structures
#[tokio::test(flavor = "current_thread")]
async fn test_json_and_deno_configs_equivalent() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create file structure
    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("main.rs"), "fn main() {}").unwrap();

    // Test with JSON config
    let json_config = r#"{
        "nodes": [
            {
                "path": "src/main.rs",
                "existence": "required",
                "kind": "file"
            }
        ]
    }"#;
    fs::write(root.join("test.json"), json_config).unwrap();

    let json_config_path = root.join("test.json");
    let json_violations = mikofia_deno::check_from_config_path(&json_config_path, root)
        .await
        .unwrap();

    // Test with Deno config
    let deno_config = r#"
        export default {
            nodes: [
                {
                    path: "src/main.rs",
                    existence: "required",
                    kind: "file"
                }
            ]
        };
    "#;
    fs::write(root.join("test.js"), deno_config).unwrap();

    let deno_config_path = root.join("test.js");
    let deno_violations = mikofia_deno::check_from_config_path(&deno_config_path, root)
        .await
        .unwrap();

    // Both should have no violations
    assert!(json_violations.is_empty());
    assert!(deno_violations.is_empty());
}
