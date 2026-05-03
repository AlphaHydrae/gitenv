use gitenv::{
    ApplyOperationOutcome, ApplyOperationReport, ConflictPolicy, FileOperation, OperationAction,
    OperationPlan, ProgramError, apply_operation_plan,
};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use tempfile::TempDir;

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
            outcomes: vec![ApplyOperationOutcome::AppliedSymlink(FileOperation {
                source: source.clone(),
                target: target.clone(),
                mkdir: true,
                conflict_policy: ConflictPolicy::Skip,
            })],
        }
    );
    assert_eq!(
        fs::read_link(&target).expect("target symlink should be readable"),
        source
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
                FileOperation {
                    source,
                    target: target.clone(),
                    mkdir: true,
                    conflict_policy: ConflictPolicy::Skip,
                }
            )],
        }
    );
    assert_eq!(
        fs::read_link(&target).expect("existing target symlink should remain readable"),
        current
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
            outcomes: vec![ApplyOperationOutcome::AppliedSymlink(FileOperation {
                source: source.clone(),
                target: target.clone(),
                mkdir: true,
                conflict_policy: ConflictPolicy::Overwrite,
            })],
        }
    );
    assert_eq!(
        fs::read_link(&target).expect("overwritten target symlink should be readable"),
        source
    );
}

#[cfg(unix)]
#[test]
fn backup_then_overwrite_existing_target_when_conflict_policy_is_overwrite_with_backup() {
    let temp = TempDir::new().expect("temporary directory should be created");
    let source = temp.path().join("source.txt");
    let current = temp.path().join("current.txt");
    let target = temp.path().join("target.txt");
    let backup = temp.path().join("target.txt.orig");
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
            outcomes: vec![ApplyOperationOutcome::AppliedSymlink(FileOperation {
                source: source.clone(),
                target: target.clone(),
                mkdir: true,
                conflict_policy: ConflictPolicy::OverwriteWithBackup,
            })],
        }
    );
    assert_eq!(
        fs::read_link(&target).expect("overwritten target symlink should be readable"),
        source
    );
    assert_eq!(
        fs::read_link(&backup).expect("backup symlink should be readable"),
        current
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
    assert_eq!(
        fs::read_link(&target).expect("existing target symlink should remain readable"),
        current
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
            outcomes: vec![ApplyOperationOutcome::AppliedSymlink(FileOperation {
                source: source.clone(),
                target: target.clone(),
                mkdir: true,
                conflict_policy: ConflictPolicy::Skip,
            })],
        }
    );
    assert_eq!(
        fs::read_link(&target).expect("target symlink should be readable"),
        source
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
        ProgramError::CreateSymlink { .. } => {}
        other => panic!("expected create symlink failure, got {other:?}"),
    }
}

#[test]
fn mark_copy_operations_as_unsupported_in_the_minimal_apply_slice() {
    let operation_plan = OperationPlan {
        actions: vec![copy_operation(
            std::path::PathBuf::from("/repo/source.txt"),
            std::path::PathBuf::from("/home/target.txt"),
        )],
    };

    let report = apply_operation_plan(&operation_plan)
        .expect("copy operations should not fail the whole apply pass yet");

    assert_eq!(
        report,
        ApplyOperationReport {
            outcomes: vec![ApplyOperationOutcome::UnsupportedOperation(copy_operation(
                std::path::PathBuf::from("/repo/source.txt"),
                std::path::PathBuf::from("/home/target.txt"),
            ),)],
        }
    );
}
