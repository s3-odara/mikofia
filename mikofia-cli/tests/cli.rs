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
