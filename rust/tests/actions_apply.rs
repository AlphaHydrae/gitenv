use gitenv::{
    ApplyOperationOutcome, ApplyOperationReport, ConflictPolicy, FileOperation, OperationAction,
    OperationPlan, apply_operation_plan,
};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use tempfile::TempDir;

fn symlink_operation(source: std::path::PathBuf, target: std::path::PathBuf) -> OperationAction {
    OperationAction::Symlink(FileOperation {
        source,
        target,
        mkdir: true,
        conflict_policy: ConflictPolicy::Skip,
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
        actions: vec![symlink_operation(source.clone(), target.clone())],
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
fn skip_existing_symlink_target_until_conflict_handling_is_implemented() {
    let temp = TempDir::new().expect("temporary directory should be created");
    let source = temp.path().join("source.txt");
    let current = temp.path().join("current.txt");
    let target = temp.path().join("target.txt");
    fs::write(&source, "source\n").expect("source file should be written");
    fs::write(&current, "current\n").expect("current file should be written");
    symlink(&current, &target).expect("existing target symlink should be created");

    let operation_plan = OperationPlan {
        actions: vec![symlink_operation(source.clone(), target.clone())],
    };

    let report = apply_operation_plan(&operation_plan)
        .expect("existing targets should be skipped in the minimal apply slice");

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
