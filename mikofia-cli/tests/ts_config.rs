use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;

fn write_ts_config(root: &Path) {
    let config = r#"
        interface Config {
            nodes: Array<{
                path: string;
                existence?: "required" | "optional" | "forbidden";
                kind?: "file" | "directory" | "any";
                rules?: Array<(ctx: any) => any>;
            }>;
        }

        const config: Config = {
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

        export default config;
    "#;

    fs::write(root.join("mikofia.config.ts"), config).unwrap();
}

#[test]
fn cli_uses_typescript_config_by_default() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path();

    write_ts_config(root);
    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("bad_case.ts"), "// demo").unwrap();

    Command::new(assert_cmd::cargo::cargo_bin!("mikofia-cli"))
        .arg("--dir")
        .arg(root)
        .assert()
        .failure()
        .stdout(predicate::str::contains("Unexpected filename: bad_case.ts"))
        .stdout(predicate::str::contains("src/bad_case.ts"))
        .code(1);
}

#[test]
fn cli_prefers_typescript_over_javascript() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path();

    // Create both .ts and .js configs
    write_ts_config(root);

    let js_config = r#"
        export default {
            nodes: [
                {
                    path: "should_not_use_this.txt",
                    existence: "required",
                    kind: "file",
                }
            ]
        };
    "#;
    fs::write(root.join("mikofia.config.js"), js_config).unwrap();

    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("bad_case.ts"), "// demo").unwrap();

    // Should use .ts config and detect the violation
    Command::new(assert_cmd::cargo::cargo_bin!("mikofia-cli"))
        .arg("--dir")
        .arg(root)
        .assert()
        .failure()
        .stdout(predicate::str::contains("Unexpected filename: bad_case.ts"))
        .code(1);
}
