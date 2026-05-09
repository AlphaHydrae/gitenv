use gitenv::{
    ApplyOperationOutcome, ApplyOperationReport, ConflictPolicy, FileOperation, OperationAction,
    OperationPlan, ProgramError, apply_operation_plan,
};
mod support;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use support::{DirectoryEntry, snapshot_directory_contents};
use tempfile::TempDir;

fn directory(path: &str) -> DirectoryEntry {
    DirectoryEntry::Directory {
        path: PathBuf::from(path),
    }
}

fn file(path: &str, contents: &str) -> DirectoryEntry {
    DirectoryEntry::File {
        path: PathBuf::from(path),
        contents: contents.to_string(),
    }
}

#[cfg(unix)]
fn symlink_entry(path: &str, target: &Path) -> DirectoryEntry {
    DirectoryEntry::Symlink {
        path: PathBuf::from(path),
        target: target.to_path_buf(),
    }
}

fn assert_temp_directory_state(temp: &TempDir, expected: Vec<DirectoryEntry>) {
    let snapshot =
        snapshot_directory_contents(temp.path()).expect("temporary directory state should be read");
    assert_eq!(snapshot, expected);
}

fn symlink_operation(
    source: std::path::PathBuf,
    target: std::path::PathBuf,
    mkdir: bool,
    conflict_policy: ConflictPolicy,
) -> OperationAction {
    OperationAction::Symlink(FileOperation {
        source,
        target,
        mkdir,
        conflict_policy,
    })
}

fn copy_operation(source: std::path::PathBuf, target: std::path::PathBuf) -> OperationAction {
    OperationAction::Copy(FileOperation {
        source,
        target,
        mkdir: true,
        conflict_policy: ConflictPolicy::Skip,
    })
}

fn copy_operation_with_options(
    source: std::path::PathBuf,
    target: std::path::PathBuf,
    mkdir: bool,
    conflict_policy: ConflictPolicy,
) -> OperationAction {
    OperationAction::Copy(FileOperation {
        source,
        target,
        mkdir,
        conflict_policy,
    })
}

#[cfg(unix)]
#[test]
fn create_symlink_when_target_is_missing() {
    let temp = TempDir::new().expect("temporary directory should be created");
    let source = temp.path().join("source.txt");
    let target = temp.path().join("target.txt");
    fs::write(&source, "source\n").expect("source file should be written");

    let operation_plan = OperationPlan {
        actions: vec![symlink_operation(
            source.clone(),
            target.clone(),
            true,
            ConflictPolicy::Skip,
        )],
    };

    let report = apply_operation_plan(&operation_plan)
        .expect("missing-target symlink operations should be applied");

    assert_eq!(
        report,
        ApplyOperationReport {
            outcomes: vec![ApplyOperationOutcome::Applied(symlink_operation(
                source.clone(),
                target.clone(),
                true,
                ConflictPolicy::Skip,
            ))],
        }
    );
    assert_temp_directory_state(
        &temp,
        vec![
            file("source.txt", "source\n"),
            symlink_entry("target.txt", &source),
        ],
    );
}

#[cfg(unix)]
#[test]
fn skip_existing_symlink_target_when_conflict_policy_is_skip() {
    let temp = TempDir::new().expect("temporary directory should be created");
    let source = temp.path().join("source.txt");
    let current = temp.path().join("current.txt");
    let target = temp.path().join("target.txt");
    fs::write(&source, "source\n").expect("source file should be written");
    fs::write(&current, "current\n").expect("current file should be written");
    symlink(&current, &target).expect("existing target symlink should be created");

    let operation_plan = OperationPlan {
        actions: vec![symlink_operation(
            source.clone(),
            target.clone(),
            true,
            ConflictPolicy::Skip,
        )],
    };

    let report =
        apply_operation_plan(&operation_plan).expect("skip conflicts should preserve targets");

    assert_eq!(
        report,
        ApplyOperationReport {
            outcomes: vec![ApplyOperationOutcome::SkippedExistingTarget(
                OperationAction::Symlink(FileOperation {
                    source,
                    target: target.clone(),
                    mkdir: true,
                    conflict_policy: ConflictPolicy::Skip,
                })
            )],
        }
    );
    assert_temp_directory_state(
        &temp,
        vec![
            file("current.txt", "current\n"),
            file("source.txt", "source\n"),
            symlink_entry("target.txt", &current),
        ],
    );
}

#[cfg(unix)]
#[test]
fn overwrite_existing_target_when_conflict_policy_is_overwrite() {
    let temp = TempDir::new().expect("temporary directory should be created");
    let source = temp.path().join("source.txt");
    let current = temp.path().join("current.txt");
    let target = temp.path().join("target.txt");
    fs::write(&source, "source\n").expect("source file should be written");
    fs::write(&current, "current\n").expect("current file should be written");
    symlink(&current, &target).expect("existing target symlink should be created");

    let operation_plan = OperationPlan {
        actions: vec![symlink_operation(
            source.clone(),
            target.clone(),
            true,
            ConflictPolicy::Overwrite,
        )],
    };

    let report =
        apply_operation_plan(&operation_plan).expect("overwrite conflicts should replace target");

    assert_eq!(
        report,
        ApplyOperationReport {
            outcomes: vec![ApplyOperationOutcome::Applied(symlink_operation(
                source.clone(),
                target.clone(),
                true,
                ConflictPolicy::Overwrite,
            ))],
        }
    );
    assert_temp_directory_state(
        &temp,
        vec![
            file("current.txt", "current\n"),
            file("source.txt", "source\n"),
            symlink_entry("target.txt", &source),
        ],
    );
}

#[cfg(unix)]
#[test]
fn backup_then_overwrite_existing_target_when_conflict_policy_is_overwrite_with_backup() {
    let temp = TempDir::new().expect("temporary directory should be created");
    let source = temp.path().join("source.txt");
    let current = temp.path().join("current.txt");
    let target = temp.path().join("target.txt");
    fs::write(&source, "source\n").expect("source file should be written");
    fs::write(&current, "current\n").expect("current file should be written");
    symlink(&current, &target).expect("existing target symlink should be created");

    let operation_plan = OperationPlan {
        actions: vec![symlink_operation(
            source.clone(),
            target.clone(),
            true,
            ConflictPolicy::OverwriteWithBackup,
        )],
    };

    let report = apply_operation_plan(&operation_plan)
        .expect("overwrite-with-backup conflicts should preserve a backup");

    assert_eq!(
        report,
        ApplyOperationReport {
            outcomes: vec![ApplyOperationOutcome::Applied(symlink_operation(
                source.clone(),
                target.clone(),
                true,
                ConflictPolicy::OverwriteWithBackup,
            ))],
        }
    );
    assert_temp_directory_state(
        &temp,
        vec![
            file("current.txt", "current\n"),
            file("source.txt", "source\n"),
            symlink_entry("target.txt", &source),
            symlink_entry("target.txt.orig", &current),
        ],
    );
}

#[cfg(unix)]
#[test]
fn reject_backup_overwrite_when_backup_path_already_exists() {
    let temp = TempDir::new().expect("temporary directory should be created");
    let source = temp.path().join("source.txt");
    let current = temp.path().join("current.txt");
    let target = temp.path().join("target.txt");
    let backup = temp.path().join("target.txt.orig");
    fs::write(&source, "source\n").expect("source file should be written");
    fs::write(&current, "current\n").expect("current file should be written");
    fs::write(&backup, "backup\n").expect("backup file should be written");
    symlink(&current, &target).expect("existing target symlink should be created");

    let operation_plan = OperationPlan {
        actions: vec![symlink_operation(
            source,
            target.clone(),
            true,
            ConflictPolicy::OverwriteWithBackup,
        )],
    };

    let error = apply_operation_plan(&operation_plan)
        .expect_err("backup conflicts should fail when backup path already exists");

    assert_eq!(error, ProgramError::BackupAlreadyExists { path: backup });
    assert_temp_directory_state(
        &temp,
        vec![
            file("current.txt", "current\n"),
            file("source.txt", "source\n"),
            symlink_entry("target.txt", &current),
            file("target.txt.orig", "backup\n"),
        ],
    );
}

#[cfg(unix)]
#[test]
fn create_missing_target_parent_directory_when_mkdir_is_enabled() {
    let temp = TempDir::new().expect("temporary directory should be created");
    let source = temp.path().join("source.txt");
    let target = temp.path().join("nested").join("config").join("target.txt");
    fs::write(&source, "source\n").expect("source file should be written");

    let operation_plan = OperationPlan {
        actions: vec![symlink_operation(
            source.clone(),
            target.clone(),
            true,
            ConflictPolicy::Skip,
        )],
    };

    let report = apply_operation_plan(&operation_plan)
        .expect("mkdir-enabled operations should create missing parent directories");

    assert_eq!(
        report,
        ApplyOperationReport {
            outcomes: vec![ApplyOperationOutcome::Applied(symlink_operation(
                source.clone(),
                target.clone(),
                true,
                ConflictPolicy::Skip,
            ))],
        }
    );
    assert_temp_directory_state(
        &temp,
        vec![
            directory("nested"),
            directory("nested/config"),
            symlink_entry("nested/config/target.txt", &source),
            file("source.txt", "source\n"),
        ],
    );
}

#[cfg(unix)]
#[test]
fn fail_when_target_parent_directory_is_missing_and_mkdir_is_disabled() {
    let temp = TempDir::new().expect("temporary directory should be created");
    let source = temp.path().join("source.txt");
    let target = temp.path().join("nested").join("config").join("target.txt");
    fs::write(&source, "source\n").expect("source file should be written");

    let operation_plan = OperationPlan {
        actions: vec![symlink_operation(
            source,
            target,
            false,
            ConflictPolicy::Skip,
        )],
    };

    let error = apply_operation_plan(&operation_plan)
        .expect_err("mkdir-disabled operations should fail when parent is missing");

    match error {
        ProgramError::SymlinkCreationFailed { .. } => {}
        other => panic!("expected create symlink failure, got {other:?}"),
    }

    assert_temp_directory_state(&temp, vec![file("source.txt", "source\n")]);
}

#[test]
fn create_copy_when_target_is_missing() {
    let temp = TempDir::new().expect("temporary directory should be created");
    let source = temp.path().join("source.txt");
    let target = temp.path().join("target.txt");
    fs::write(&source, "source\n").expect("source file should be written");

    let operation_plan = OperationPlan {
        actions: vec![copy_operation(source.clone(), target.clone())],
    };

    let report = apply_operation_plan(&operation_plan)
        .expect("missing-target copy operations should be applied");

    assert_eq!(
        report,
        ApplyOperationReport {
            outcomes: vec![ApplyOperationOutcome::Applied(copy_operation(
                source.clone(),
                target.clone(),
            ))],
        }
    );
    assert_temp_directory_state(
        &temp,
        vec![
            file("source.txt", "source\n"),
            file("target.txt", "source\n"),
        ],
    );
}

#[test]
fn skip_existing_copy_target_when_conflict_policy_is_skip() {
    let temp = TempDir::new().expect("temporary directory should be created");
    let source = temp.path().join("source.txt");
    let target = temp.path().join("target.txt");
    fs::write(&source, "source\n").expect("source file should be written");
    fs::write(&target, "current\n").expect("target file should be written");

    let operation_plan = OperationPlan {
        actions: vec![copy_operation_with_options(
            source.clone(),
            target.clone(),
            true,
            ConflictPolicy::Skip,
        )],
    };

    let report =
        apply_operation_plan(&operation_plan).expect("skip conflicts should preserve copy targets");

    assert_eq!(
        report,
        ApplyOperationReport {
            outcomes: vec![ApplyOperationOutcome::SkippedExistingTarget(
                copy_operation_with_options(source, target.clone(), true, ConflictPolicy::Skip,)
            )],
        }
    );
    assert_temp_directory_state(
        &temp,
        vec![
            file("source.txt", "source\n"),
            file("target.txt", "current\n"),
        ],
    );
}

#[test]
fn overwrite_existing_copy_target_when_conflict_policy_is_overwrite() {
    let temp = TempDir::new().expect("temporary directory should be created");
    let source = temp.path().join("source.txt");
    let target = temp.path().join("target.txt");
    fs::write(&source, "source\n").expect("source file should be written");
    fs::write(&target, "current\n").expect("target file should be written");

    let operation_plan = OperationPlan {
        actions: vec![copy_operation_with_options(
            source.clone(),
            target.clone(),
            true,
            ConflictPolicy::Overwrite,
        )],
    };

    let report = apply_operation_plan(&operation_plan)
        .expect("overwrite conflicts should replace copy targets");

    assert_eq!(
        report,
        ApplyOperationReport {
            outcomes: vec![ApplyOperationOutcome::Applied(copy_operation_with_options(
                source.clone(),
                target.clone(),
                true,
                ConflictPolicy::Overwrite,
            ))],
        }
    );
    assert_temp_directory_state(
        &temp,
        vec![
            file("source.txt", "source\n"),
            file("target.txt", "source\n"),
        ],
    );
}

#[test]
fn backup_then_overwrite_existing_copy_target_when_conflict_policy_is_overwrite_with_backup() {
    let temp = TempDir::new().expect("temporary directory should be created");
    let source = temp.path().join("source.txt");
    let target = temp.path().join("target.txt");
    fs::write(&source, "source\n").expect("source file should be written");
    fs::write(&target, "current\n").expect("target file should be written");

    let operation_plan = OperationPlan {
        actions: vec![copy_operation_with_options(
            source.clone(),
            target.clone(),
            true,
            ConflictPolicy::OverwriteWithBackup,
        )],
    };

    let report = apply_operation_plan(&operation_plan)
        .expect("overwrite-with-backup conflicts should preserve a copy backup");

    assert_eq!(
        report,
        ApplyOperationReport {
            outcomes: vec![ApplyOperationOutcome::Applied(copy_operation_with_options(
                source.clone(),
                target.clone(),
                true,
                ConflictPolicy::OverwriteWithBackup,
            ))],
        }
    );
    assert_temp_directory_state(
        &temp,
        vec![
            file("source.txt", "source\n"),
            file("target.txt", "source\n"),
            file("target.txt.orig", "current\n"),
        ],
    );
}

#[test]
fn fail_when_copy_target_parent_directory_is_missing_and_mkdir_is_disabled() {
    let temp = TempDir::new().expect("temporary directory should be created");
    let source = temp.path().join("source.txt");
    let target = temp.path().join("nested").join("config").join("target.txt");
    fs::write(&source, "source\n").expect("source file should be written");

    let operation_plan = OperationPlan {
        actions: vec![copy_operation_with_options(
            source,
            target,
            false,
            ConflictPolicy::Skip,
        )],
    };

    let error = apply_operation_plan(&operation_plan)
        .expect_err("mkdir-disabled copy operations should fail when parent is missing");

    match error {
        ProgramError::FileCopyFailed { .. } => {}
        other => panic!("expected copy-file failure, got {other:?}"),
    }

    assert_temp_directory_state(&temp, vec![file("source.txt", "source\n")]);
}

#[test]
fn create_missing_copy_target_parent_directory_when_mkdir_is_enabled() {
    let temp = TempDir::new().expect("temporary directory should be created");
    let source = temp.path().join("source.txt");
    let target = temp.path().join("nested").join("config").join("target.txt");
    fs::write(&source, "source\n").expect("source file should be written");

    let operation_plan = OperationPlan {
        actions: vec![copy_operation_with_options(
            source.clone(),
            target.clone(),
            true,
            ConflictPolicy::Skip,
        )],
    };

    let report = apply_operation_plan(&operation_plan)
        .expect("mkdir-enabled copy operations should create missing parent directories");

    assert_eq!(
        report,
        ApplyOperationReport {
            outcomes: vec![ApplyOperationOutcome::Applied(copy_operation_with_options(
                source.clone(),
                target.clone(),
                true,
                ConflictPolicy::Skip,
            ))],
        }
    );
    assert_temp_directory_state(
        &temp,
        vec![
            directory("nested"),
            directory("nested/config"),
            file("nested/config/target.txt", "source\n"),
            file("source.txt", "source\n"),
        ],
    );
}

#[test]
fn reject_backup_overwrite_for_copy_when_backup_path_already_exists() {
    let temp = TempDir::new().expect("temporary directory should be created");
    let source = temp.path().join("source.txt");
    let target = temp.path().join("target.txt");
    let backup = temp.path().join("target.txt.orig");
    fs::write(&source, "source\n").expect("source file should be written");
    fs::write(&target, "current\n").expect("target file should be written");
    fs::write(&backup, "backup\n").expect("backup file should be written");

    let operation_plan = OperationPlan {
        actions: vec![copy_operation_with_options(
            source,
            target.clone(),
            true,
            ConflictPolicy::OverwriteWithBackup,
        )],
    };

    let error = apply_operation_plan(&operation_plan)
        .expect_err("copy backup conflicts should fail when backup path already exists");

    assert_eq!(error, ProgramError::BackupAlreadyExists { path: backup });
    assert_temp_directory_state(
        &temp,
        vec![
            file("source.txt", "source\n"),
            file("target.txt", "current\n"),
            file("target.txt.orig", "backup\n"),
        ],
    );
}

#[cfg(unix)]
#[test]
fn preserve_operation_kind_in_apply_outcomes_for_symlink_and_copy_actions() {
    let temp = TempDir::new().expect("temporary directory should be created");
    let symlink_source = temp.path().join("source-link.txt");
    let symlink_target = temp.path().join("target-link.txt");
    let copy_source = temp.path().join("source-copy.txt");
    let copy_target = temp.path().join("target-copy.txt");
    fs::write(&symlink_source, "link\n").expect("symlink source file should be written");
    fs::write(&copy_source, "copy\n").expect("copy source file should be written");

    let operation_plan = OperationPlan {
        actions: vec![
            symlink_operation(
                symlink_source.clone(),
                symlink_target.clone(),
                true,
                ConflictPolicy::Skip,
            ),
            copy_operation(copy_source.clone(), copy_target.clone()),
        ],
    };

    let report = apply_operation_plan(&operation_plan)
        .expect("apply should preserve operation kinds in typed outcomes");

    assert_eq!(
        report,
        ApplyOperationReport {
            outcomes: vec![
                ApplyOperationOutcome::Applied(symlink_operation(
                    symlink_source.clone(),
                    symlink_target.clone(),
                    true,
                    ConflictPolicy::Skip,
                )),
                ApplyOperationOutcome::Applied(copy_operation(
                    copy_source.clone(),
                    copy_target.clone(),
                )),
            ],
        }
    );
    assert_temp_directory_state(
        &temp,
        vec![
            file("source-copy.txt", "copy\n"),
            file("source-link.txt", "link\n"),
            file("target-copy.txt", "copy\n"),
            symlink_entry("target-link.txt", &symlink_source),
        ],
    );
}
