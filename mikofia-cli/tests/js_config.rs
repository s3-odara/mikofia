use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;

fn write_js_config(root: &Path) {
    let config = r#"
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

    fs::write(root.join("mikofia.config.js"), config).unwrap();
}

#[test]
fn cli_uses_javascript_config_by_default() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path();

    write_js_config(root);
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
