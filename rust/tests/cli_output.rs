use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn gitenv_command_for_home(home: &TempDir) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gitenv"));
    command
        .env("HOME", home.path())
        // Keep tests deterministic when CI sets global config path variables.
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("GITENV_CONFIG")
        .env_remove("GITENV_REPO")
        .env_remove("GITENV_COLOR")
        .env_remove("GITENV_LOG_LEVEL")
        .env_remove("COLOR");
    command
}

fn create_repository_with_gitconfig() -> TempDir {
    let repository = TempDir::new().expect("temporary repository should be created");
    fs::write(repository.path().join(".gitconfig"), "[user]\n")
        .expect("source file should be written");
    repository
}

fn write_minimal_config(home: &TempDir, repository: &Path) {
    let config = format!(
        concat!(
            "version: 1\n",
            "repository: \"{}\"\n",
            "sources:\n",
            "  - from: \".\"\n",
            "    configs:\n",
            "      - file: .gitconfig\n"
        ),
        repository.display()
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
}

// Integration tests here cover only CLI-related paths not covered by unit
// tests: exit codes, the stderr/stdout boundary, and end-to-end wiring. Output
// content and command dispatch logic are covered by unit tests which are faster
// and do not require spawning a subprocess.

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

#[test]
fn fail_when_home_is_missing() {
    let home = TempDir::new().expect("temporary home directory should be created");

    let output = gitenv_command_for_home(&home)
        .env_remove("HOME")
        .output()
        .expect("binary should run");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot resolve home directory from $HOME"));
}

#[test]
fn fail_when_repository_override_is_whitespace_only() {
    let home = TempDir::new().expect("temporary home directory should be created");
    let repository = create_repository_with_gitconfig();
    write_minimal_config(&home, repository.path());

    let output = gitenv_command_for_home(&home)
        .env("GITENV_REPO", "   ")
        .output()
        .expect("binary should run");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("repository root selected from env precedence source cannot be empty"));
}

// Log level flags

#[test]
fn emit_debug_log_lines_to_stderr_when_log_level_is_debug() {
    let home = TempDir::new().expect("temporary home directory should be created");
    let repository = create_repository_with_gitconfig();
    write_minimal_config(&home, repository.path());

    let output = gitenv_command_for_home(&home)
        .args(["--log-level", "debug"])
        .output()
        .expect("binary should run");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Debug-level output should include structured log lines from domain modules.
    assert!(
        !stderr.is_empty(),
        "stderr should contain debug log lines when --log-level debug is set"
    );
    assert!(
        stderr.contains("[DEBUG]") || stderr.contains("[INFO]") || stderr.contains("[TRACE]"),
        "stderr should contain bracketed log level labels; got: {stderr}"
    );
}

#[test]
fn produce_no_log_output_on_stderr_at_default_log_level() {
    let home = TempDir::new().expect("temporary home directory should be created");
    let repository = create_repository_with_gitconfig();
    write_minimal_config(&home, repository.path());

    // Default log level is warn; normal operations emit no warnings.
    let output = gitenv_command_for_home(&home)
        .output()
        .expect("binary should run");

    assert!(output.status.success());
    assert!(
        output.stderr.is_empty(),
        "stderr should be empty at default log level (warn)"
    );
}

#[test]
fn show_the_repository_precedence_source_in_debug_logs() {
    let home = TempDir::new().expect("temporary home directory should be created");
    let config_repository = create_repository_with_gitconfig();
    let env_repository = create_repository_with_gitconfig();
    write_minimal_config(&home, config_repository.path());

    let output = gitenv_command_for_home(&home)
        .args(["--log-level", "debug"])
        .env("GITENV_REPO", env_repository.path())
        .output()
        .expect("binary should run");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("resolved repository root from env precedence source"),
        "stderr should include the selected repository precedence source at debug level; got: {stderr}"
    );
}

#[test]
fn emit_debug_log_lines_to_stderr_when_log_level_is_set_by_env() {
    let home = TempDir::new().expect("temporary home directory should be created");
    let repository = create_repository_with_gitconfig();
    write_minimal_config(&home, repository.path());

    let output = gitenv_command_for_home(&home)
        .env("GITENV_LOG_LEVEL", "debug")
        .output()
        .expect("binary should run");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.is_empty(),
        "stderr should contain debug log lines when GITENV_LOG_LEVEL=debug"
    );
    assert!(
        stderr.contains("[DEBUG]") || stderr.contains("[INFO]") || stderr.contains("[TRACE]"),
        "stderr should contain bracketed log level labels; got: {stderr}"
    );
}

#[test]
fn force_colorized_output_when_gitenv_color_yes_is_set() {
    let home = TempDir::new().expect("temporary home directory should be created");
    let repository = create_repository_with_gitconfig();
    write_minimal_config(&home, repository.path());

    let output = gitenv_command_for_home(&home)
        .env("GITENV_COLOR", "yes")
        .output()
        .expect("binary should run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\u{1b}["),
        "stdout should include ANSI color escapes when GITENV_COLOR=yes; got: {stdout}"
    );
}

#[test]
fn ignore_the_legacy_color_environment_variable_name() {
    let home = TempDir::new().expect("temporary home directory should be created");
    let repository = create_repository_with_gitconfig();
    write_minimal_config(&home, repository.path());

    let output = gitenv_command_for_home(&home)
        .env("COLOR", "yes")
        .output()
        .expect("binary should run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("\u{1b}["),
        "stdout should stay uncolored when only legacy COLOR is set; got: {stdout}"
    );
}
