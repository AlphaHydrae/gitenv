use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn gitenv_command_for_home(home: &TempDir) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gitenv"));
    command
        .env("HOME", home.path())
        // Keep tests deterministic when CI sets global config path variables.
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("GITENV_CONFIG");
    command
}

// Integration tests here cover only what requires a running binary process:
// exit codes, the stderr/stdout boundary, and end-to-end wiring. Output
// content and command dispatch logic are covered by unit tests in lib.rs and
// cli.rs, which are faster and do not require spawning a subprocess.

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

    let output = gitenv_command_for_home(&home)
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
fn show_the_apply_subcommand_exits_successfully_and_outputs_to_stdout() {
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

    let output = gitenv_command_for_home(&home)
        .arg("apply")
        .output()
        .expect("binary should run");

    assert!(output.status.success());
    assert!(!output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn fail_the_default_inspection_when_the_config_file_is_missing() {
    let home = TempDir::new().expect("temporary home directory should be created");

    let output = gitenv_command_for_home(&home)
        .output()
        .expect("binary should run");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("gitenv: cannot read config at"));
    assert!(stderr.contains("Config file locations"));
}
