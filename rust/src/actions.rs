//! Apply execution stage.
//!
//! Executes an [`OperationPlan`] against the filesystem, creating symlinks and
//! copies as directed. Returns a structured [`ApplyOperationReport`] recording
//! the per-operation outcome.
//!
//! The composition root in `lib.rs` wires a real [`TargetProbe`] and
//! [`SymlinkCreator`] boundary into an [`ApplyContext`]. Tests supply local
//! closure-based doubles that record calls or return pre-configured results
//! without touching the real filesystem.

use crate::{
    FileOperation, OperationAction, OperationPlan, ProgramError,
    boundary::{SymlinkCreator, TargetProbe},
    logging,
    status::{CopyInspectionState, inspect_copy_operation_status},
};
use log::Level;
use std::path::Path;

const BACKUP_SUFFIX: &str = ".orig";

/// Structured apply outcomes for one operation-plan execution pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyOperationReport {
    pub outcomes: Vec<ApplyOperationOutcome>,
}

/// Per-operation apply result for incremental symlink execution support.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyOperationOutcome {
    Applied(OperationAction),
    SkippedExistingTarget(OperationAction),
    UnsupportedOperation(OperationAction),
}

/// Stage-owned context for apply execution, carrying trait-backed dependency
/// references for filesystem probes and symlink creation.
pub(crate) struct ApplyContext<'a> {
    pub(crate) target_probe: &'a dyn TargetProbe,
    pub(crate) symlink_creator: &'a dyn SymlinkCreator,
}

/// Internal apply entrypoint accepting a stage-owned context for deterministic
/// tests and composition-root injection.
pub(crate) fn apply_operation_plan(
    operation_plan: &OperationPlan,
    context: &ApplyContext,
) -> Result<ApplyOperationReport, ProgramError> {
    logging::actions(
        Level::Info,
        "apply_operation_plan_start",
        format!("actions={}", operation_plan.actions.len()),
    );

    let mut outcomes = Vec::new();

    for action in &operation_plan.actions {
        let outcome = match action {
            OperationAction::Symlink(operation) => apply_symlink_operation(operation, context)?,
            OperationAction::Copy(operation) => {
                apply_copy_operation(operation, context.target_probe)?
            }
        };

        logging::actions(
            Level::Debug,
            "apply_action_processed",
            format!("kind={}", action_kind_from_outcome(&outcome)),
        );

        outcomes.push(outcome);
    }

    logging::actions(
        Level::Info,
        "apply_operation_plan_success",
        format!("outcomes={}", outcomes.len()),
    );

    Ok(ApplyOperationReport { outcomes })
}

fn action_kind_from_outcome(outcome: &ApplyOperationOutcome) -> &'static str {
    match outcome {
        ApplyOperationOutcome::Applied(OperationAction::Symlink(_))
        | ApplyOperationOutcome::SkippedExistingTarget(OperationAction::Symlink(_))
        | ApplyOperationOutcome::UnsupportedOperation(OperationAction::Symlink(_)) => "symlink",
        ApplyOperationOutcome::Applied(OperationAction::Copy(_))
        | ApplyOperationOutcome::SkippedExistingTarget(OperationAction::Copy(_))
        | ApplyOperationOutcome::UnsupportedOperation(OperationAction::Copy(_)) => "copy",
    }
}

pub(crate) fn target_exists(path: &Path) -> Result<bool, ProgramError> {
    logging::system(
        Level::Trace,
        "symlink_metadata",
        format!("path={}", path.display()),
    );

    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(ProgramError::PathInspectionFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        }),
    }
}

fn apply_symlink_operation(
    operation: &FileOperation,
    context: &ApplyContext,
) -> Result<ApplyOperationOutcome, ProgramError> {
    if operation.mkdir {
        ensure_parent_directory_exists(&operation.target)?;
    }

    if !context.target_probe.target_exists(&operation.target)? {
        context
            .symlink_creator
            .create_symlink(&operation.source, &operation.target)?;
        return Ok(ApplyOperationOutcome::Applied(OperationAction::Symlink(
            operation.clone(),
        )));
    }

    match operation.conflict_policy {
        crate::ConflictPolicy::Skip => Ok(ApplyOperationOutcome::SkippedExistingTarget(
            OperationAction::Symlink(operation.clone()),
        )),
        crate::ConflictPolicy::Overwrite => {
            remove_target_path(&operation.target)?;
            context
                .symlink_creator
                .create_symlink(&operation.source, &operation.target)?;
            Ok(ApplyOperationOutcome::Applied(OperationAction::Symlink(
                operation.clone(),
            )))
        }
        crate::ConflictPolicy::OverwriteWithBackup => {
            let backup_path = backup_path_for_target(&operation.target);
            if context.target_probe.target_exists(&backup_path)? {
                return Err(ProgramError::BackupAlreadyExists { path: backup_path });
            }

            move_target_to_backup(&operation.target, &backup_path)?;
            context
                .symlink_creator
                .create_symlink(&operation.source, &operation.target)?;
            Ok(ApplyOperationOutcome::Applied(OperationAction::Symlink(
                operation.clone(),
            )))
        }
    }
}

fn apply_copy_operation(
    operation: &FileOperation,
    target_probe: &dyn TargetProbe,
) -> Result<ApplyOperationOutcome, ProgramError> {
    if operation.mkdir {
        ensure_parent_directory_exists(&operation.target)?;
    }

    if !target_probe.target_exists(&operation.target)? {
        copy_source_to_target(&operation.source, &operation.target)?;
        return Ok(ApplyOperationOutcome::Applied(OperationAction::Copy(
            operation.clone(),
        )));
    }

    if inspect_copy_operation_status(operation)?.state == CopyInspectionState::Ok {
        return Ok(ApplyOperationOutcome::SkippedExistingTarget(
            OperationAction::Copy(operation.clone()),
        ));
    }

    match operation.conflict_policy {
        crate::ConflictPolicy::Skip => Ok(ApplyOperationOutcome::SkippedExistingTarget(
            OperationAction::Copy(operation.clone()),
        )),
        crate::ConflictPolicy::Overwrite => {
            remove_target_path(&operation.target)?;
            copy_source_to_target(&operation.source, &operation.target)?;
            Ok(ApplyOperationOutcome::Applied(OperationAction::Copy(
                operation.clone(),
            )))
        }
        crate::ConflictPolicy::OverwriteWithBackup => {
            let backup_path = backup_path_for_target(&operation.target);
            if target_probe.target_exists(&backup_path)? {
                return Err(ProgramError::BackupAlreadyExists { path: backup_path });
            }

            move_target_to_backup(&operation.target, &backup_path)?;
            copy_source_to_target(&operation.source, &operation.target)?;
            Ok(ApplyOperationOutcome::Applied(OperationAction::Copy(
                operation.clone(),
            )))
        }
    }
}

fn copy_source_to_target(source: &Path, target: &Path) -> Result<(), ProgramError> {
    logging::system(
        Level::Trace,
        "copy",
        format!("source={} target={}", source.display(), target.display()),
    );

    std::fs::copy(source, target)
        .map(|_| ())
        .map_err(|error| ProgramError::FileCopyFailed {
            source: source.to_path_buf(),
            target: target.to_path_buf(),
            message: error.to_string(),
        })
}

fn ensure_parent_directory_exists(target: &Path) -> Result<(), ProgramError> {
    let Some(parent) = target.parent() else {
        return Ok(());
    };

    logging::system(
        Level::Trace,
        "create_dir_all",
        format!("path={}", parent.display()),
    );

    std::fs::create_dir_all(parent).map_err(|error| ProgramError::TargetDirectoryCreationFailed {
        path: parent.to_path_buf(),
        message: error.to_string(),
    })
}

fn remove_target_path(path: &Path) -> Result<(), ProgramError> {
    logging::system(
        Level::Trace,
        "symlink_metadata",
        format!("path={}", path.display()),
    );

    let metadata =
        std::fs::symlink_metadata(path).map_err(|error| ProgramError::TargetRemovalFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    if metadata.file_type().is_dir() {
        logging::system(
            Level::Trace,
            "remove_dir",
            format!("path={}", path.display()),
        );

        std::fs::remove_dir(path).map_err(|error| ProgramError::TargetRemovalFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })
    } else {
        logging::system(
            Level::Trace,
            "remove_file",
            format!("path={}", path.display()),
        );

        std::fs::remove_file(path).map_err(|error| ProgramError::TargetRemovalFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })
    }
}

fn move_target_to_backup(target: &Path, backup_path: &Path) -> Result<(), ProgramError> {
    logging::system(
        Level::Trace,
        "rename",
        format!(
            "source={} target={}",
            target.display(),
            backup_path.display()
        ),
    );

    std::fs::rename(target, backup_path).map_err(|error| ProgramError::TargetBackupFailed {
        path: target.to_path_buf(),
        backup_path: backup_path.to_path_buf(),
        message: error.to_string(),
    })
}

fn backup_path_for_target(path: &Path) -> std::path::PathBuf {
    let mut backup = path.as_os_str().to_os_string();
    backup.push(BACKUP_SUFFIX);
    backup.into()
}

#[cfg(unix)]
pub(crate) fn create_symlink_on_filesystem(
    source: &Path,
    target: &Path,
) -> Result<(), ProgramError> {
    logging::system(
        Level::Trace,
        "symlink",
        format!("source={} target={}", source.display(), target.display()),
    );

    std::os::unix::fs::symlink(source, target).map_err(|error| {
        ProgramError::SymlinkCreationFailed {
            source: source.to_path_buf(),
            target: target.to_path_buf(),
            message: error.to_string(),
        }
    })
}

#[cfg(not(unix))]
pub(crate) fn create_symlink_on_filesystem(
    source: &Path,
    target: &Path,
) -> Result<(), ProgramError> {
    Err(ProgramError::SymlinkCreationFailed {
        source: source.to_path_buf(),
        target: target.to_path_buf(),
        message: "symlink apply is not supported on this platform".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        ApplyContext, apply_copy_operation, apply_operation_plan, backup_path_for_target,
        ensure_parent_directory_exists, move_target_to_backup, remove_target_path, target_exists,
    };
    use crate::{
        ApplyOperationOutcome, ConflictPolicy, FileOperation, OperationAction, OperationPlan,
        ProgramError,
        boundary::{
            TargetProbe,
            test_doubles::{FnSymlinkCreator, FnTargetProbe},
        },
    };
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    struct NativeTargetProbe;

    impl TargetProbe for NativeTargetProbe {
        fn target_exists(&self, path: &Path) -> Result<bool, ProgramError> {
            target_exists(path)
        }
    }

    fn symlink_plan(
        source: PathBuf,
        target: PathBuf,
        mkdir: bool,
        conflict_policy: ConflictPolicy,
    ) -> OperationPlan {
        OperationPlan {
            actions: vec![OperationAction::Symlink(FileOperation {
                source,
                target,
                mkdir,
                conflict_policy,
            })],
        }
    }

    #[test]
    fn append_orig_suffix_to_backup_paths() {
        assert_eq!(
            backup_path_for_target(std::path::Path::new("/home/.gitconfig")),
            PathBuf::from("/home/.gitconfig.orig")
        );
    }

    #[test]
    fn allow_targets_without_a_parent_path() {
        ensure_parent_directory_exists(std::path::Path::new(""))
            .expect("empty targets should not require mkdir");
    }

    #[test]
    fn report_create_directory_failures_with_typed_errors() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let parent_file = temp.path().join("parent-as-file");
        fs::write(&parent_file, "x\n").expect("parent file should be written");
        let nested_target = parent_file.join("target.txt");

        let error = ensure_parent_directory_exists(&nested_target)
            .expect_err("mkdir should fail when parent path is a file");

        assert!(matches!(
            &error,
            ProgramError::TargetDirectoryCreationFailed { path, .. } if path == &parent_file
        ));
    }

    #[test]
    fn report_remove_target_metadata_failures() {
        let missing = PathBuf::from("/path/that/does/not/exist");

        let error = remove_target_path(&missing)
            .expect_err("remove should fail when target metadata cannot be read");

        assert!(matches!(
            &error,
            ProgramError::TargetRemovalFailed { path, .. } if path == &missing
        ));
    }

    #[test]
    fn report_remove_target_failures_for_non_empty_directories() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let target_directory = temp.path().join("target-directory");
        fs::create_dir_all(&target_directory).expect("target directory should be created");
        fs::write(target_directory.join("nested.txt"), "nested\n")
            .expect("nested file should be written");

        let error = remove_target_path(&target_directory)
            .expect_err("remove should fail for non-empty directories");

        assert!(matches!(
            &error,
            ProgramError::TargetRemovalFailed { path, .. } if path == &target_directory
        ));
    }

    #[cfg(unix)]
    #[test]
    fn report_remove_target_failures_for_files_in_non_writable_directories() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let protected_directory = temp.path().join("protected");
        fs::create_dir_all(&protected_directory).expect("protected directory should be created");
        let target_file = protected_directory.join("target.txt");
        fs::write(&target_file, "target\n").expect("target file should be written");

        let mut permissions = fs::metadata(&protected_directory)
            .expect("protected directory metadata should be readable")
            .permissions();
        permissions.set_mode(0o500);
        fs::set_permissions(&protected_directory, permissions)
            .expect("protected directory should be set to read-only");

        let error = remove_target_path(&target_file)
            .expect_err("remove should fail when parent directory is not writable");

        let mut restore_permissions = fs::metadata(&protected_directory)
            .expect("protected directory metadata should be readable")
            .permissions();
        restore_permissions.set_mode(0o700);
        fs::set_permissions(&protected_directory, restore_permissions)
            .expect("protected directory permissions should be restored");

        assert!(matches!(
            &error,
            ProgramError::TargetRemovalFailed { path, .. } if path == &target_file
        ));
    }

    #[test]
    fn report_backup_move_failures_with_typed_errors() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let target = temp.path().join("target.txt");
        let backup = temp.path().join("target.txt.orig");

        let error = move_target_to_backup(&target, &backup)
            .expect_err("backup move should fail when the source target is missing");

        assert!(matches!(
            &error,
            ProgramError::TargetBackupFailed {
                path,
                backup_path,
                ..
            } if path == &target && backup_path == &backup
        ));
    }

    #[cfg(unix)]
    #[test]
    fn report_metadata_probe_failures_when_parent_directory_is_not_accessible() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let locked_directory = temp.path().join("locked");
        fs::create_dir_all(&locked_directory).expect("locked directory should be created");
        let target = locked_directory.join("target.txt");

        let mut permissions = fs::metadata(&locked_directory)
            .expect("locked directory metadata should be readable")
            .permissions();
        permissions.set_mode(0o000);
        fs::set_permissions(&locked_directory, permissions)
            .expect("locked directory should be set to inaccessible");

        let error = target_exists(&target)
            .expect_err("metadata probe should fail for inaccessible directory entries");

        let mut restore_permissions = fs::metadata(&locked_directory)
            .expect("locked directory metadata should be readable")
            .permissions();
        restore_permissions.set_mode(0o700);
        fs::set_permissions(&locked_directory, restore_permissions)
            .expect("locked directory permissions should be restored");

        assert!(matches!(
            &error,
            ProgramError::PathInspectionFailed { path, .. } if path == &target
        ));
    }

    #[test]
    fn cannot_apply_operation_plan_when_target_probe_fails() {
        let operation_plan = symlink_plan(
            PathBuf::from("/repo/source"),
            PathBuf::from("/home/target"),
            true,
            ConflictPolicy::Skip,
        );
        let target_probe = FnTargetProbe(|path| {
            Err(ProgramError::PathInspectionFailed {
                path: path.to_path_buf(),
                message: "permission denied".to_string(),
            })
        });
        let symlink_creator = FnSymlinkCreator(super::create_symlink_on_filesystem);
        let context = ApplyContext {
            target_probe: &target_probe,
            symlink_creator: &symlink_creator,
        };

        let error = apply_operation_plan(&operation_plan, &context)
            .expect_err("apply should propagate target probe failures");

        assert_eq!(
            error,
            ProgramError::PathInspectionFailed {
                path: PathBuf::from("/home/target"),
                message: "permission denied".to_string(),
            }
        );
    }

    #[test]
    fn apply_a_missing_symlink_through_the_stage_context() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source = temp.path().join("source.txt");
        let target = temp.path().join("target.txt");
        let operation_plan =
            symlink_plan(source.clone(), target.clone(), false, ConflictPolicy::Skip);
        let target_probe = FnTargetProbe(|_| Ok(false));
        let symlink_creator = FnSymlinkCreator(super::create_symlink_on_filesystem);
        let context = ApplyContext {
            target_probe: &target_probe,
            symlink_creator: &symlink_creator,
        };

        fs::write(&source, "source\n").expect("source file should be written");

        let report = apply_operation_plan(&operation_plan, &context)
            .expect("apply context should create a missing symlink target");

        assert_eq!(
            report,
            crate::ApplyOperationReport {
                outcomes: vec![ApplyOperationOutcome::Applied(OperationAction::Symlink(
                    FileOperation {
                        source: source.clone(),
                        target: target.clone(),
                        mkdir: false,
                        conflict_policy: ConflictPolicy::Skip,
                    },
                ))],
            }
        );
        assert_eq!(
            std::fs::read_link(&target).expect("created symlink target should be readable"),
            source
        );
    }

    #[test]
    fn skip_copy_overwrite_when_target_contents_already_match() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source = temp.path().join("source.txt");
        let target = temp.path().join("target.txt");
        let target_probe = NativeTargetProbe;
        fs::write(&source, "same\n").expect("source file should be written");
        fs::write(&target, "same\n").expect("target file should be written");

        let outcome = apply_copy_operation(
            &FileOperation {
                source: source.clone(),
                target: target.clone(),
                mkdir: true,
                conflict_policy: ConflictPolicy::Overwrite,
            },
            &target_probe,
        )
        .expect("matching copy targets should be skipped");

        assert_eq!(
            outcome,
            ApplyOperationOutcome::SkippedExistingTarget(OperationAction::Copy(FileOperation {
                source,
                target,
                mkdir: true,
                conflict_policy: ConflictPolicy::Overwrite,
            }))
        );
    }

    #[test]
    fn overwrite_copy_when_target_contents_differ() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source = temp.path().join("source.txt");
        let target = temp.path().join("target.txt");
        let target_probe = NativeTargetProbe;
        fs::write(&source, "source\n").expect("source file should be written");
        fs::write(&target, "target\n").expect("target file should be written");

        let outcome = apply_copy_operation(
            &FileOperation {
                source: source.clone(),
                target: target.clone(),
                mkdir: true,
                conflict_policy: ConflictPolicy::Overwrite,
            },
            &target_probe,
        )
        .expect("differing copy targets should be overwritten");

        assert_eq!(
            outcome,
            ApplyOperationOutcome::Applied(OperationAction::Copy(FileOperation {
                source,
                target: target.clone(),
                mkdir: true,
                conflict_policy: ConflictPolicy::Overwrite,
            }))
        );
        assert_eq!(
            fs::read_to_string(&target).expect("overwritten copy target should be readable"),
            "source\n"
        );
    }

    #[test]
    fn report_copy_failures() {
        let operation_plan = OperationPlan {
            actions: vec![OperationAction::Copy(FileOperation {
                source: PathBuf::from("/repo/source"),
                target: PathBuf::from("/home/target"),
                mkdir: true,
                conflict_policy: ConflictPolicy::Skip,
            })],
        };
        let target_probe = FnTargetProbe(|_| Ok(false));
        let symlink_creator = FnSymlinkCreator(|_, _| Ok(()));
        let context = ApplyContext {
            target_probe: &target_probe,
            symlink_creator: &symlink_creator,
        };

        let error = apply_operation_plan(&operation_plan, &context)
            .expect_err("copy actions should fail when copy creation cannot run");

        assert!(matches!(
            &error,
            ProgramError::FileCopyFailed { source, target, .. }
                if source == &PathBuf::from("/repo/source")
                    && target == &PathBuf::from("/home/target")
        ));
    }
}
