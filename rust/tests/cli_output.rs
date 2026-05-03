use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn show_the_default_inspection_output_for_a_missing_symlink() {
    let home = TempDir::new().expect("temporary home directory should be created");
    let repository = TempDir::new().expect("temporary repository should be created");
    fs::write(repository.path().join(".gitconfig"), "[user]\n")
        .expect("source file should be written");

    let config = format!(
        concat!(
            "version: 1\n",
            "repository: \"{}\"\n",
            "sources:\n",
            "  - from: \".\"\n",
            "    configs:\n",
            "      - file: .gitconfig\n"
        ),
        repository.path().display()
    );
    let config_path = home
        .path()
        .join(".config")
        .join("gitenv")
        .join("config.yml");
    fs::create_dir_all(
        config_path
            .parent()
            .expect("config directory should have a parent"),
    )
    .expect("config directory should be created");
    fs::write(config_path, config).expect("config file should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_gitenv"))
        .env("HOME", home.path())
        .output()
        .expect("binary should run");

    let expected_stdout = format!(
        "{} -> {}   not yet set up\n",
        home.path().join(".gitconfig").display(),
        repository.path().join(".").join(".gitconfig").display()
    );

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected_stdout);
    assert!(output.stderr.is_empty());
}

#[test]
fn fail_the_default_inspection_when_the_config_file_is_missing() {
    let home = TempDir::new().expect("temporary home directory should be created");

    let output = Command::new(env!("CARGO_BIN_EXE_gitenv"))
        .env("HOME", home.path())
        .output()
        .expect("binary should run");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("gitenv: cannot read config at"));
    assert!(stderr.contains("Config file locations"));
}
