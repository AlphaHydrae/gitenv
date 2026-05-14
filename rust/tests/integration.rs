use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

mod support;
use support::{DirectoryEntry, snapshot_directory_contents};

#[cfg(unix)]
use std::os::unix::fs::symlink;

fn directory(path: &str) -> DirectoryEntry {
    DirectoryEntry::Directory { path: path.into() }
}

fn file(path: &str, contents: &str) -> DirectoryEntry {
    DirectoryEntry::File {
        path: path.into(),
        contents: contents.to_string(),
    }
}

#[cfg(unix)]
fn symlink_entry(path: &str, target: &Path) -> DirectoryEntry {
    DirectoryEntry::Symlink {
        path: path.into(),
        target: target.to_path_buf(),
    }
}

fn gitenv_command_for_home(home: &TempDir) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gitenv"));
    command
        .env("HOME", home.path())
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("GITENV_CONFIG");
    command
}

fn write_file(path: &Path, contents: &str) {
    fs::create_dir_all(
        path.parent()
            .expect("test file paths should always have a parent directory"),
    )
    .expect("parent directory should be created");
    fs::write(path, contents).expect("test file should be written");
}

fn replace_home_prefix_with_tilde(output: String, home: &Path) -> String {
    output.replace(&format!("{}/", home.display()), "~/")
}

// These broad integration tests intentionally cover behavior that is also
// asserted in unit tests. Their job is different: exercise the compiled binary
// with real config loading, planning, rendering, and filesystem wiring in a
// few readable end-to-end scenarios.

#[cfg(unix)]
#[test]
fn invoke_the_info_command() {
    let home = TempDir::new().expect("temporary home directory should be created");
    let repository = TempDir::new().expect("temporary repository should be created");

    write_file(
        &repository.path().join("missing-zshrc"),
        "export PATH=~/bin:$PATH\n",
    );
    write_file(
        &repository.path().join("copied-missing.conf"),
        "[missing-copy]\n",
    );
    write_file(&repository.path().join("linked-ok.conf"), "linked ok\n");
    write_file(&repository.path().join("copied-ok.conf"), "copied ok\n");
    write_file(
        &repository.path().join("file-instead-of-link.conf"),
        "expected link\n",
    );
    write_file(
        &repository.path().join("points-expected.conf"),
        "expected destination\n",
    );
    write_file(
        &repository.path().join("copied-differs.conf"),
        "expected copy contents\n",
    );
    write_file(
        &repository.path().join("copied-not-a-file.conf"),
        "copy target should not be a directory\n",
    );
    write_file(
        &repository.path().join("profiles").join(".aliases"),
        "alias gs='git status'\n",
    );
    write_file(
        &repository.path().join("profiles").join(".profile"),
        "export EDITOR=vim\n",
    );
    write_file(
        &repository.path().join("profiles").join(".ignored"),
        "ignored\n",
    );
    write_file(
        &repository.path().join("profiles").join("README"),
        "not selected\n",
    );
    write_file(
        &repository.path().join("guarded-file.conf"),
        "guarded content\n",
    );
    write_file(
        &repository.path().join("custom-to-file.conf"),
        "custom to file\n",
    );
    let shared_config = format!(
        concat!(
            "version: 1\n",
            "repository: \"{}\"\n",
            "sources:\n",
            "  - from: \".\"\n",
            "    to: \".local/share/gitenv-info-shared\"\n",
            "    configs:\n",
            "      - file: shared-source.conf\n",
            "        as: shared/config.conf\n",
            "        mode: copy\n"
        ),
        repository.path().display()
    );
    write_file(&repository.path().join("shared.yml"), &shared_config);
    write_file(
        &repository.path().join("shared-source.conf"),
        "shared config content\n",
    );

    let config = format!(
        concat!(
            "version: 1\n",
            "repository: \"{}\"\n",
            "includes:\n",
            "  - \"{}/shared.yml\"\n",
            "defaults:\n",
            "  mkdir: true\n",
            "sources:\n",
            "  - from: \".\"\n",
            "    to: \".config/gitenv-info\"\n",
            "    configs:\n",
            "      - file: missing-zshrc\n",
            "        as: missing/.zshrc\n",
            "      - file: copied-missing.conf\n",
            "        as: copy/missing.conf\n",
            "        mode: copy\n",
            "      - file: linked-ok.conf\n",
            "        as: linked/ok.conf\n",
            "      - file: copied-ok.conf\n",
            "        as: copy/ok.conf\n",
            "        mode: copy\n",
            "      - file: file-instead-of-link.conf\n",
            "        as: linked/not-a-link.conf\n",
            "      - file: points-expected.conf\n",
            "        as: linked/points-elsewhere.conf\n",
            "      - file: copied-differs.conf\n",
            "        as: copy/differs.conf\n",
            "        mode: copy\n",
            "      - file: copied-not-a-file.conf\n",
            "        as: copy/not-a-file.conf\n",
            "        mode: copy\n",
            "  - from: profiles\n",
            "    to: \".local/share/gitenv-info/profiles\"\n",
            "    configs:\n",
            "      - select:\n",
            "          type: non-dot\n",
            "          exclude:\n",
            "            - .ignored\n",
            "  - from: \".\"\n",
            "    to: \".local/share/gitenv-info-guarded\"\n",
            "    when: to_exists\n",
            "    configs:\n",
            "      - file: guarded-file.conf\n",
            "        as: guarded.conf\n",
            "  - from: \".\"\n",
            "    to: \".local/share/gitenv-info-custom\"\n",
            "    configs:\n",
            "      - file: custom-to-file.conf\n",
            "        as: custom.conf\n"
        ),
        repository.path().display(),
        repository.path().display()
    );
    let config_path = home
        .path()
        .join(".config")
        .join("gitenv")
        .join("config.yml");
    write_file(&config_path, &config);

    let linked_ok_target = home
        .path()
        .join(".config")
        .join("gitenv-info")
        .join("linked")
        .join("ok.conf");
    let copied_ok_target = home
        .path()
        .join(".config")
        .join("gitenv-info")
        .join("copy")
        .join("ok.conf");
    let not_a_link_target = home
        .path()
        .join(".config")
        .join("gitenv-info")
        .join("linked")
        .join("not-a-link.conf");
    let points_elsewhere_target = home
        .path()
        .join(".config")
        .join("gitenv-info")
        .join("linked")
        .join("points-elsewhere.conf");
    let differs_target = home
        .path()
        .join(".config")
        .join("gitenv-info")
        .join("copy")
        .join("differs.conf");
    let not_a_file_target = home
        .path()
        .join(".config")
        .join("gitenv-info")
        .join("copy")
        .join("not-a-file.conf");
    let selected_profile_target = home
        .path()
        .join(".local")
        .join("share")
        .join("gitenv-info")
        .join("profiles")
        .join(".profile");
    let selected_readme_target = home
        .path()
        .join(".local")
        .join("share")
        .join("gitenv-info")
        .join("profiles")
        .join("README");
    let shared_config_target = home
        .path()
        .join(".local")
        .join("share")
        .join("gitenv-info-shared")
        .join("shared")
        .join("config.conf");
    let custom_to_target = home
        .path()
        .join(".local")
        .join("share")
        .join("gitenv-info-custom")
        .join("custom.conf");
    let elsewhere_target = repository.path().join("elsewhere.conf");

    fs::create_dir_all(
        linked_ok_target
            .parent()
            .expect("linked ok target should have a parent"),
    )
    .expect("linked ok target parent should be created");
    symlink(repository.path().join("linked-ok.conf"), &linked_ok_target)
        .expect("linked ok target should be created");

    write_file(&copied_ok_target, "copied ok\n");
    write_file(&not_a_link_target, "this is a regular file\n");
    write_file(&elsewhere_target, "elsewhere\n");
    symlink(&elsewhere_target, &points_elsewhere_target)
        .expect("points elsewhere target should be created");
    write_file(&differs_target, "different contents\n");
    fs::create_dir_all(&not_a_file_target).expect("not-a-file target directory should be created");
    fs::create_dir_all(
        selected_profile_target
            .parent()
            .expect("selected profile target should have a parent"),
    )
    .expect("selected profile target parent should be created");
    symlink(
        repository.path().join("profiles").join(".profile"),
        &selected_profile_target,
    )
    .expect("selected profile target should be created");
    fs::create_dir_all(
        selected_readme_target
            .parent()
            .expect("selected readme target should have a parent"),
    )
    .expect("selected readme target parent should be created");
    // README doesn't exist in home yet; gitenv should show it as "not yet set up"

    let home_snapshot_before =
        snapshot_directory_contents(home.path()).expect("home directory should be readable");

    let output = gitenv_command_for_home(&home)
        .output()
        .expect("binary should run");

    let expected_stdout = replace_home_prefix_with_tilde(
        format!(
            concat!(
                "{} -> {}   not yet set up\n",
                "{} <- {}   not yet set up\n",
                "{} -> {}   ok\n",
                "{} <- {}   ok\n",
                "{} -> {}   not a symlink\n",
                "{} -> {}   points to {}\n",
                "{} <- {}   differs from source\n",
                "{} <- {}   not a file\n",
                "{} -> {}   not yet set up\n",
                "{} -> {}   not yet set up\n",
                "{} <- {}   not yet set up\n",
            ),
            home.path()
                .join(".config")
                .join("gitenv-info")
                .join("missing")
                .join(".zshrc")
                .display(),
            repository.path().join(".").join("missing-zshrc").display(),
            home.path()
                .join(".config")
                .join("gitenv-info")
                .join("copy")
                .join("missing.conf")
                .display(),
            repository
                .path()
                .join(".")
                .join("copied-missing.conf")
                .display(),
            linked_ok_target.display(),
            repository.path().join(".").join("linked-ok.conf").display(),
            copied_ok_target.display(),
            repository.path().join(".").join("copied-ok.conf").display(),
            not_a_link_target.display(),
            repository
                .path()
                .join(".")
                .join("file-instead-of-link.conf")
                .display(),
            points_elsewhere_target.display(),
            repository
                .path()
                .join(".")
                .join("points-expected.conf")
                .display(),
            elsewhere_target.display(),
            differs_target.display(),
            repository
                .path()
                .join(".")
                .join("copied-differs.conf")
                .display(),
            not_a_file_target.display(),
            repository
                .path()
                .join(".")
                .join("copied-not-a-file.conf")
                .display(),
            home.path()
                .join(".local")
                .join("share")
                .join("gitenv-info")
                .join("profiles")
                .join("README")
                .display(),
            repository.path().join("profiles").join("README").display(),
            custom_to_target.display(),
            repository
                .path()
                .join(".")
                .join("custom-to-file.conf")
                .display(),
            shared_config_target.display(),
            repository
                .path()
                .join(".")
                .join("shared-source.conf")
                .display()
        ),
        home.path(),
    );

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected_stdout);
    assert!(output.stderr.is_empty());

    let home_snapshot_after =
        snapshot_directory_contents(home.path()).expect("home directory should be readable");
    assert_eq!(home_snapshot_after, home_snapshot_before);
}

#[cfg(unix)]
#[test]
fn invoke_the_apply_command() {
    let home = TempDir::new().expect("temporary home directory should be created");
    let repository = TempDir::new().expect("temporary repository should be created");
    let env_source = TempDir::new().expect("temporary env source directory should be created");

    write_file(
        &repository.path().join("create-link.conf"),
        "created link\n",
    );
    write_file(
        &repository.path().join("create-copy.conf"),
        "created copy\n",
    );
    write_file(&repository.path().join("keep-link.conf"), "keep link\n");
    write_file(&repository.path().join("keep-copy.conf"), "keep copy\n");
    write_file(
        &repository.path().join("overwrite-link.conf"),
        "overwrite link\n",
    );
    write_file(
        &repository.path().join("backup-copy.conf"),
        "new copy contents\n",
    );
    write_file(
        &repository.path().join("match-copy.conf"),
        "matching copy contents\n",
    );
    write_file(
        &repository.path().join("profiles").join(".aliases"),
        "alias gs='git status'\n",
    );
    write_file(
        &repository.path().join("profiles").join(".profile"),
        "export EDITOR=vim\n",
    );
    write_file(
        &repository.path().join("profiles").join(".ignored"),
        "ignored\n",
    );
    write_file(
        &repository.path().join("profiles").join("README"),
        "not selected\n",
    );
    write_file(
        &env_source.path().join("env-sourced.conf"),
        "from env source\n",
    );
    let shared_config = format!(
        concat!(
            "version: 1\n",
            "repository: \"{}\"\n",
            "sources:\n",
            "  - from: \".\"\n",
            "    to: \".local/share/gitenv-apply-shared\"\n",
            "    configs:\n",
            "      - file: shared-source.conf\n",
            "        as: shared/config.conf\n",
            "        mode: copy\n"
        ),
        repository.path().display()
    );
    write_file(&repository.path().join("shared.yml"), &shared_config);
    write_file(
        &repository.path().join("shared-source.conf"),
        "shared config content\n",
    );

    let config = format!(
        concat!(
            "version: 1\n",
            "repository: \"{}\"\n",
            "includes:\n",
            "  - \"{}/shared.yml\"\n",
            "defaults:\n",
            "  mkdir: true\n",
            "sources:\n",
            "  - from: \".\"\n",
            "    to: \".config/gitenv-apply\"\n",
            "    configs:\n",
            "      - file: create-link.conf\n",
            "        as: create/link.conf\n",
            "      - file: create-copy.conf\n",
            "        as: create/copy.conf\n",
            "        mode: copy\n",
            "      - file: keep-link.conf\n",
            "        as: keep/link.conf\n",
            "      - file: keep-copy.conf\n",
            "        as: keep/copy.conf\n",
            "        mode: copy\n",
            "      - file: overwrite-link.conf\n",
            "        as: replace/link.conf\n",
            "        overwrite: true\n",
            "      - file: backup-copy.conf\n",
            "        as: replace/copy.conf\n",
            "        mode: copy\n",
            "        overwrite: true\n",
            "        backup_on_overwrite: true\n",
            "      - file: match-copy.conf\n",
            "        as: replace/match.conf\n",
            "        mode: copy\n",
            "        overwrite: true\n",
            "        backup_on_overwrite: true\n",
            "  - from: profiles\n",
            "    to: \".local/share/gitenv-apply/profiles\"\n",
            "    configs:\n",
            "      - select:\n",
            "          type: dot\n",
            "          exclude:\n",
            "            - .ignored\n",
            "  - from: $GITENV_TEST_SOURCE_DIR\n",
            "    to: \".local/share/gitenv-apply-env\"\n",
            "    configs:\n",
            "      - file: env-sourced.conf\n",
            "        as: env.conf\n"
        ),
        repository.path().display(),
        repository.path().display()
    );
    let config_path = home
        .path()
        .join(".config")
        .join("gitenv")
        .join("config.yml");
    write_file(&config_path, &config);

    let keep_link_target = home
        .path()
        .join(".config")
        .join("gitenv-apply")
        .join("keep")
        .join("link.conf");
    let keep_copy_target = home
        .path()
        .join(".config")
        .join("gitenv-apply")
        .join("keep")
        .join("copy.conf");
    let overwrite_link_target = home
        .path()
        .join(".config")
        .join("gitenv-apply")
        .join("replace")
        .join("link.conf");
    let backup_copy_target = home
        .path()
        .join(".config")
        .join("gitenv-apply")
        .join("replace")
        .join("copy.conf");
    let match_copy_target = home
        .path()
        .join(".config")
        .join("gitenv-apply")
        .join("replace")
        .join("match.conf");
    let selected_profile_target = home
        .path()
        .join(".local")
        .join("share")
        .join("gitenv-apply")
        .join("profiles")
        .join(".profile");
    let shared_config_target = home
        .path()
        .join(".local")
        .join("share")
        .join("gitenv-apply-shared")
        .join("shared")
        .join("config.conf");
    let env_sourced_target = home
        .path()
        .join(".local")
        .join("share")
        .join("gitenv-apply-env")
        .join("env.conf");

    fs::create_dir_all(
        keep_link_target
            .parent()
            .expect("keep link target should have a parent"),
    )
    .expect("keep link target parent should be created");
    symlink(repository.path().join("keep-link.conf"), &keep_link_target)
        .expect("keep link target should be created");
    write_file(&keep_copy_target, "keep copy\n");
    write_file(&overwrite_link_target, "replace me\n");
    write_file(&backup_copy_target, "old copy contents\n");
    write_file(&match_copy_target, "matching copy contents\n");
    fs::create_dir_all(
        selected_profile_target
            .parent()
            .expect("selected profile target should have a parent"),
    )
    .expect("selected profile target parent should be created");
    symlink(
        repository.path().join("profiles").join(".profile"),
        &selected_profile_target,
    )
    .expect("selected profile target should be created");

    let output = gitenv_command_for_home(&home)
        .env("GITENV_TEST_SOURCE_DIR", env_source.path())
        .arg("apply")
        .output()
        .expect("binary should run");

    let create_link_target = home
        .path()
        .join(".config")
        .join("gitenv-apply")
        .join("create")
        .join("link.conf");
    let create_copy_target = home
        .path()
        .join(".config")
        .join("gitenv-apply")
        .join("create")
        .join("copy.conf");
    let selected_aliases_target = home
        .path()
        .join(".local")
        .join("share")
        .join("gitenv-apply")
        .join("profiles")
        .join(".aliases");

    let expected_stdout = replace_home_prefix_with_tilde(
        format!(
            concat!(
                "created symlink {} -> {}\n",
                "copied {} to {}\n",
                "skipped symlink {} (already exists)\n",
                "skipped copy {} (already exists)\n",
                "created symlink {} -> {}\n",
                "copied {} to {}\n",
                "skipped copy {} (already exists)\n",
                "created symlink {} -> {}\n",
                "skipped symlink {} (already exists)\n",
                "created symlink {} -> {}\n",
                "copied {} to {}\n",
            ),
            create_link_target.display(),
            repository
                .path()
                .join(".")
                .join("create-link.conf")
                .display(),
            repository
                .path()
                .join(".")
                .join("create-copy.conf")
                .display(),
            create_copy_target.display(),
            keep_link_target.display(),
            keep_copy_target.display(),
            overwrite_link_target.display(),
            repository
                .path()
                .join(".")
                .join("overwrite-link.conf")
                .display(),
            repository
                .path()
                .join(".")
                .join("backup-copy.conf")
                .display(),
            backup_copy_target.display(),
            match_copy_target.display(),
            selected_aliases_target.display(),
            repository
                .path()
                .join("profiles")
                .join(".aliases")
                .display(),
            selected_profile_target.display(),
            env_sourced_target.display(),
            env_source.path().join("env-sourced.conf").display(),
            repository
                .path()
                .join(".")
                .join("shared-source.conf")
                .display(),
            shared_config_target.display()
        ),
        home.path(),
    );

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected_stdout);
    assert!(output.stderr.is_empty());

    let home_snapshot =
        snapshot_directory_contents(home.path()).expect("home directory should be readable");
    assert_eq!(
        home_snapshot,
        vec![
            directory(".config"),
            directory(".config/gitenv"),
            file(".config/gitenv/config.yml", &config),
            directory(".config/gitenv-apply"),
            directory(".config/gitenv-apply/create"),
            file(".config/gitenv-apply/create/copy.conf", "created copy\n"),
            symlink_entry(
                ".config/gitenv-apply/create/link.conf",
                &repository.path().join(".").join("create-link.conf"),
            ),
            directory(".config/gitenv-apply/keep"),
            file(".config/gitenv-apply/keep/copy.conf", "keep copy\n"),
            symlink_entry(
                ".config/gitenv-apply/keep/link.conf",
                &repository.path().join("keep-link.conf"),
            ),
            directory(".config/gitenv-apply/replace"),
            file(
                ".config/gitenv-apply/replace/copy.conf",
                "new copy contents\n"
            ),
            file(
                ".config/gitenv-apply/replace/copy.conf.orig",
                "old copy contents\n",
            ),
            symlink_entry(
                ".config/gitenv-apply/replace/link.conf",
                &repository.path().join(".").join("overwrite-link.conf"),
            ),
            file(
                ".config/gitenv-apply/replace/link.conf.orig",
                "replace me\n"
            ),
            file(
                ".config/gitenv-apply/replace/match.conf",
                "matching copy contents\n",
            ),
            directory(".local"),
            directory(".local/share"),
            directory(".local/share/gitenv-apply"),
            directory(".local/share/gitenv-apply/profiles"),
            symlink_entry(
                ".local/share/gitenv-apply/profiles/.aliases",
                &repository.path().join("profiles").join(".aliases"),
            ),
            symlink_entry(
                ".local/share/gitenv-apply/profiles/.profile",
                &repository.path().join("profiles").join(".profile"),
            ),
            directory(".local/share/gitenv-apply-env"),
            symlink_entry(
                ".local/share/gitenv-apply-env/env.conf",
                &env_source.path().join("env-sourced.conf"),
            ),
            directory(".local/share/gitenv-apply-shared"),
            directory(".local/share/gitenv-apply-shared/shared"),
            file(
                ".local/share/gitenv-apply-shared/shared/config.conf",
                "shared config content\n",
            ),
        ]
    );
}

#[cfg(unix)]
#[test]
fn show_info_output_when_config_uses_relative_include_paths() {
    let home = TempDir::new().expect("temporary home directory should be created");
    let repository = TempDir::new().expect("temporary repository should be created");

    write_file(&repository.path().join("root.conf"), "root\n");
    write_file(&repository.path().join("shared.conf"), "shared\n");

    let root_config = format!(
        concat!(
            "version: 1\n",
            "repository: \"{}\"\n",
            "includes:\n",
            "  - includes/shared.yml\n",
            "sources:\n",
            "  - from: \".\"\n",
            "    configs:\n",
            "      - file: root.conf\n",
            "        as: .root.conf\n"
        ),
        repository.path().display()
    );
    let shared_config = format!(
        concat!(
            "version: 1\n",
            "repository: \"{}\"\n",
            "sources:\n",
            "  - from: \".\"\n",
            "    configs:\n",
            "      - file: shared.conf\n",
            "        as: .shared.conf\n"
        ),
        repository.path().display()
    );

    let config_directory = home.path().join(".config").join("gitenv");
    write_file(&config_directory.join("config.yml"), &root_config);
    write_file(
        &config_directory.join("includes").join("shared.yml"),
        &shared_config,
    );

    let home_snapshot_before =
        snapshot_directory_contents(home.path()).expect("home directory should be readable");

    let unrelated_working_directory = TempDir::new().expect("temporary working directory");
    let output = gitenv_command_for_home(&home)
        .current_dir(unrelated_working_directory.path())
        .output()
        .expect("binary should run");

    let expected_stdout = replace_home_prefix_with_tilde(
        format!(
            concat!("{} -> {}   not yet set up\n", "{} -> {}   not yet set up\n"),
            home.path().join(".root.conf").display(),
            repository.path().join(".").join("root.conf").display(),
            home.path().join(".shared.conf").display(),
            repository.path().join(".").join("shared.conf").display(),
        ),
        home.path(),
    );

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected_stdout);
    assert!(output.stderr.is_empty());

    let home_snapshot_after =
        snapshot_directory_contents(home.path()).expect("home directory should be readable");
    assert_eq!(home_snapshot_after, home_snapshot_before);
}

#[cfg(unix)]
#[test]
fn apply_recursive_select_entries_into_nested_targets() {
    let home = TempDir::new().expect("temporary home directory should be created");
    let repository = TempDir::new().expect("temporary repository should be created");

    write_file(
        &repository.path().join("bundle").join("a-root.conf"),
        "root\n",
    );
    write_file(
        &repository
            .path()
            .join("bundle")
            .join("nested")
            .join("deep.conf"),
        "deep\n",
    );
    write_file(
        &repository.path().join("bundle").join("z-last.conf"),
        "last\n",
    );

    let config = format!(
        concat!(
            "version: 1\n",
            "repository: \"{}\"\n",
            "defaults:\n",
            "  mkdir: true\n",
            "sources:\n",
            "  - from: bundle\n",
            "    to: \".config/gitenv-recursive\"\n",
            "    configs:\n",
            "      - select:\n",
            "          type: all\n",
            "          recursive: true\n"
        ),
        repository.path().display()
    );
    let config_path = home
        .path()
        .join(".config")
        .join("gitenv")
        .join("config.yml");
    write_file(&config_path, &config);

    let output = gitenv_command_for_home(&home)
        .arg("apply")
        .output()
        .expect("binary should run");

    let expected_stdout = replace_home_prefix_with_tilde(
        format!(
            concat!(
                "created symlink {} -> {}\n",
                "created symlink {} -> {}\n",
                "created symlink {} -> {}\n",
            ),
            home.path()
                .join(".config")
                .join("gitenv-recursive")
                .join("a-root.conf")
                .display(),
            repository
                .path()
                .join("bundle")
                .join("a-root.conf")
                .display(),
            home.path()
                .join(".config")
                .join("gitenv-recursive")
                .join("nested")
                .join("deep.conf")
                .display(),
            repository
                .path()
                .join("bundle")
                .join("nested")
                .join("deep.conf")
                .display(),
            home.path()
                .join(".config")
                .join("gitenv-recursive")
                .join("z-last.conf")
                .display(),
            repository
                .path()
                .join("bundle")
                .join("z-last.conf")
                .display(),
        ),
        home.path(),
    );

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected_stdout);
    assert!(output.stderr.is_empty());

    let home_snapshot = snapshot_directory_contents(home.path())
        .expect("home directory should be readable after recursive apply");
    assert_eq!(
        home_snapshot,
        vec![
            directory(".config"),
            directory(".config/gitenv"),
            file(".config/gitenv/config.yml", &config),
            directory(".config/gitenv-recursive"),
            symlink_entry(
                ".config/gitenv-recursive/a-root.conf",
                &repository.path().join("bundle").join("a-root.conf"),
            ),
            directory(".config/gitenv-recursive/nested"),
            symlink_entry(
                ".config/gitenv-recursive/nested/deep.conf",
                &repository
                    .path()
                    .join("bundle")
                    .join("nested")
                    .join("deep.conf"),
            ),
            symlink_entry(
                ".config/gitenv-recursive/z-last.conf",
                &repository.path().join("bundle").join("z-last.conf"),
            ),
        ]
    );
}
