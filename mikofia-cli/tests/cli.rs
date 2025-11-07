use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::io::Write;
use tempfile::TempDir;

fn write_config(dir: &TempDir, config_body: &str) -> std::path::PathBuf {
    let path = dir.path().join("mikofia.config.json");
    let mut file = std::fs::File::create(&path).expect("create config file");
    file.write_all(config_body.as_bytes())
        .expect("write config file");
    path
}

fn write_file(dir: &TempDir, path: &str) {
    let file_path = dir.path().join(path);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(file_path, "content").expect("write file");
}

// ========== Check command tests (existing + updated) ==========

#[test]
fn fails_when_config_missing() {
    let temp = TempDir::new().expect("create temp dir");

    Command::new(assert_cmd::cargo::cargo_bin!("mikofia-cli"))
        .current_dir(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Config file not found"))
        .code(2);
}

#[test]
fn reads_config_from_dir_flag() {
    let temp = TempDir::new().expect("create temp dir");
    write_config(
        &temp,
        r#"{"nodes": [{"path": "Cargo.toml", "existence": "required"}]}"#,
    );
    write_file(&temp, "Cargo.toml");

    Command::new(assert_cmd::cargo::cargo_bin!("mikofia-cli"))
        .arg("--dir")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("All checks passed"));
}

#[test]
fn reads_config_from_custom_path() {
    let temp = TempDir::new().expect("create temp dir");
    let config_path = temp.path().join("configs").join("alt.json");
    fs::create_dir_all(config_path.parent().unwrap()).expect("create config dir");
    fs::write(
        &config_path,
        r#"{"nodes": [{"path": "Cargo.toml", "existence": "required"}]}"#,
    )
    .expect("write config");
    write_file(&temp, "Cargo.toml");

    Command::new(assert_cmd::cargo::cargo_bin!("mikofia-cli"))
        .arg("--dir")
        .arg(temp.path())
        .arg("--config")
        .arg(&config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("All checks passed"));
}

#[test]
fn reports_absolute_path_in_violation_output() {
    let temp = TempDir::new().expect("create temp dir");
    write_config(
        &temp,
        r#"{"nodes": [{"path": "missing.txt", "existence": "required"}]}"#,
    );

    let expected_path = temp.path().join("missing.txt");

    Command::new(assert_cmd::cargo::cargo_bin!("mikofia-cli"))
        .arg("--dir")
        .arg(temp.path())
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            expected_path.to_string_lossy().as_ref(),
        ))
        .stdout(predicate::str::contains(
            "Required item not found: missing.txt",
        ))
        .code(1);
}

#[test]
fn check_subcommand_works() {
    let temp = TempDir::new().expect("create temp dir");
    write_config(
        &temp,
        r#"{"nodes": [{"path": "Cargo.toml", "existence": "required"}]}"#,
    );
    write_file(&temp, "Cargo.toml");

    Command::new(assert_cmd::cargo::cargo_bin!("mikofia-cli"))
        .arg("check")
        .arg("--dir")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("All checks passed"));
}

// ========== Init command tests ==========

#[test]
fn init_creates_typescript_config() {
    let temp = TempDir::new().expect("create temp dir");

    Command::new(assert_cmd::cargo::cargo_bin!("mikofia-cli"))
        .arg("init")
        .arg("--dir")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Created config file"));

    let config_path = temp.path().join("mikofia.config.ts");
    assert!(config_path.exists());

    let content = fs::read_to_string(config_path).expect("read config file");
    assert!(content.contains("@type {import('mikofia').Config}"));
    assert!(content.contains("export default"));
    assert!(content.contains("node_modules"));
    assert!(content.contains("src"));
}

#[test]
fn init_creates_javascript_config() {
    let temp = TempDir::new().expect("create temp dir");

    Command::new(assert_cmd::cargo::cargo_bin!("mikofia-cli"))
        .arg("init")
        .arg("--format")
        .arg("js")
        .arg("--dir")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Created config file"));

    let config_path = temp.path().join("mikofia.config.js");
    assert!(config_path.exists());

    let content = fs::read_to_string(config_path).expect("read config file");
    assert!(!content.contains("@type"));
    assert!(content.contains("export default"));
    assert!(content.contains("node_modules"));
}

#[test]
fn init_creates_json_config() {
    let temp = TempDir::new().expect("create temp dir");

    Command::new(assert_cmd::cargo::cargo_bin!("mikofia-cli"))
        .arg("init")
        .arg("--format")
        .arg("json")
        .arg("--dir")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Created config file"));

    let config_path = temp.path().join("mikofia.config.json");
    assert!(config_path.exists());

    let content = fs::read_to_string(config_path).expect("read config file");
    // Validate JSON parsing
    let _parsed: serde_json::Value = serde_json::from_str(&content).expect("valid JSON");
}

#[test]
fn init_fails_when_config_exists() {
    let temp = TempDir::new().expect("create temp dir");

    // Create existing config
    write_config(&temp, r#"{"nodes": []}"#);

    Command::new(assert_cmd::cargo::cargo_bin!("mikofia-cli"))
        .arg("init")
        .arg("--dir")
        .arg(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Config file already exists"))
        .stderr(predicate::str::contains("Use --force to overwrite"))
        .code(3);
}

#[test]
fn init_force_overwrites_existing() {
    let temp = TempDir::new().expect("create temp dir");

    // Create existing config
    write_config(&temp, r#"{"nodes": []}"#);

    Command::new(assert_cmd::cargo::cargo_bin!("mikofia-cli"))
        .arg("init")
        .arg("--dir")
        .arg(temp.path())
        .arg("--force")
        .assert()
        .success()
        .stdout(predicate::str::contains("Created config file"));

    let config_path = temp.path().join("mikofia.config.ts");
    assert!(config_path.exists());

    let content = fs::read_to_string(config_path).expect("read config file");
    assert!(content.contains("export default"));
}

#[test]
fn init_custom_config_name() {
    let temp = TempDir::new().expect("create temp dir");

    Command::new(assert_cmd::cargo::cargo_bin!("mikofia-cli"))
        .arg("init")
        .arg("--config")
        .arg("custom.config.ts")
        .arg("--dir")
        .arg(temp.path())
        .assert()
        .success();

    let config_path = temp.path().join("custom.config.ts");
    assert!(config_path.exists());
}

#[test]
fn init_respects_existing_with_custom_name() {
    let temp = TempDir::new().expect("create temp dir");

    // Create custom config file
    let custom_path = temp.path().join("custom.ts");
    fs::write(&custom_path, "existing content").expect("write file");

    Command::new(assert_cmd::cargo::cargo_bin!("mikofia-cli"))
        .arg("init")
        .arg("--config")
        .arg("custom.ts")
        .arg("--dir")
        .arg(temp.path())
        .assert()
        .failure()
        .code(3);
}

#[test]
fn init_creates_parent_directories() {
    let temp = TempDir::new().expect("create temp dir");

    Command::new(assert_cmd::cargo::cargo_bin!("mikofia-cli"))
        .arg("init")
        .arg("--config")
        .arg("configs/mikofia.config.ts")
        .arg("--dir")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Created config file"));

    let config_path = temp.path().join("configs").join("mikofia.config.ts");
    assert!(config_path.exists());
}

#[test]
fn init_creates_new_directory() {
    let temp = TempDir::new().expect("create temp dir");
    let new_project = temp.path().join("new-project");

    Command::new(assert_cmd::cargo::cargo_bin!("mikofia-cli"))
        .arg("init")
        .arg("--dir")
        .arg(&new_project)
        .assert()
        .success()
        .stdout(predicate::str::contains("Created config file"));

    let config_path = new_project.join("mikofia.config.ts");
    assert!(config_path.exists());
}
