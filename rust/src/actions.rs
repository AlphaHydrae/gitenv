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
    FileOperation, OperationAction, OperationEntry, OperationPlan, ProgramError,
    boundary::{DirectoryCreator, PathRemover, SymlinkCreator, TargetProbe},
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
    SkippedMissingTargetDirectory(OperationAction),
    UnsupportedOperation(OperationAction),
}

/// Stage-owned context for apply execution, carrying trait-backed dependency
/// references for filesystem probes and symlink creation.
pub(crate) struct ApplyContext<'a> {
    pub(crate) target_probe: &'a dyn TargetProbe,
    pub(crate) symlink_creator: &'a dyn SymlinkCreator,
    pub(crate) directory_creator: &'a dyn DirectoryCreator,
    pub(crate) path_remover: &'a dyn PathRemover,
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
        format!("entries={}", operation_plan.entries.len()),
    );

    let blocking_diagnostics = operation_plan.apply_blocking_diagnostics();
    if !blocking_diagnostics.is_empty() {
        return Err(ProgramError::OperationPlanBlocked {
            diagnostics: blocking_diagnostics,
        });
    }

    let mut outcomes = Vec::new();

    for entry in &operation_plan.entries {
        let OperationEntry::Action(action_entry) = entry else {
            continue;
        };

        let action = &action_entry.action;
        if let Some(crate::operation::OperationSkipReason::MissingTargetDirectory) =
            action_entry.skip_reason
        {
            outcomes.push(ApplyOperationOutcome::SkippedMissingTargetDirectory(
                action.clone(),
            ));
            continue;
        }

        let outcome = match action {
            OperationAction::Symlink(operation) => apply_symlink_operation(operation, context)?,
            OperationAction::Copy(operation) => apply_copy_operation(operation, context)?,
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
        | ApplyOperationOutcome::SkippedMissingTargetDirectory(OperationAction::Symlink(_))
        | ApplyOperationOutcome::UnsupportedOperation(OperationAction::Symlink(_)) => "symlink",
        ApplyOperationOutcome::Applied(OperationAction::Copy(_))
        | ApplyOperationOutcome::SkippedExistingTarget(OperationAction::Copy(_))
        | ApplyOperationOutcome::SkippedMissingTargetDirectory(OperationAction::Copy(_))
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
        context
            .directory_creator
            .ensure_parent_directory_exists(&operation.target)?;
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
            context.path_remover.remove_path(&operation.target)?;
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
    context: &ApplyContext,
) -> Result<ApplyOperationOutcome, ProgramError> {
    if operation.mkdir {
        context
            .directory_creator
            .ensure_parent_directory_exists(&operation.target)?;
    }

    if !context.target_probe.target_exists(&operation.target)? {
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
            context.path_remover.remove_path(&operation.target)?;
            copy_source_to_target(&operation.source, &operation.target)?;
            Ok(ApplyOperationOutcome::Applied(OperationAction::Copy(
                operation.clone(),
            )))
        }
        crate::ConflictPolicy::OverwriteWithBackup => {
            let backup_path = backup_path_for_target(&operation.target);
            if context.target_probe.target_exists(&backup_path)? {
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
        ApplyContext, apply_copy_operation, apply_operation_plan, apply_symlink_operation,
        backup_path_for_target, move_target_to_backup, target_exists,
    };
    use crate::{
        ApplyOperationOutcome, ConflictPolicy, FileOperation, OperationAction, OperationEntry,
        OperationPlan, PlannedOperationAction, ProgramError, SourceAvailability,
        boundary::{
            RealBoundary, TargetProbe,
            test_doubles::{FnDirectoryCreator, FnPathRemover, FnSymlinkCreator, FnTargetProbe},
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
            entries: vec![OperationEntry::Action(PlannedOperationAction {
                action: OperationAction::Symlink(FileOperation {
                    source,
                    target,
                    mkdir,
                    conflict_policy,
                }),
                source_availability: SourceAvailability::Available,
                skip_reason: None,
            })],
        }
    }

    fn copy_plan(
        source: PathBuf,
        target: PathBuf,
        mkdir: bool,
        conflict_policy: ConflictPolicy,
    ) -> OperationPlan {
        OperationPlan {
            entries: vec![OperationEntry::Action(PlannedOperationAction {
                action: OperationAction::Copy(FileOperation {
                    source,
                    target,
                    mkdir,
                    conflict_policy,
                }),
                source_availability: SourceAvailability::Available,
                skip_reason: None,
            })],
        }
    }

    fn ok_directory_creation(_: &Path) -> Result<(), ProgramError> {
        Ok(())
    }

    fn ok_path_removal(_: &Path) -> Result<(), ProgramError> {
        Ok(())
    }

    fn ok_symlink_creation(_: &Path, _: &Path) -> Result<(), ProgramError> {
        Ok(())
    }

    #[test]
    fn no_op_test_boundary_helpers_return_ok() {
        assert!(ok_directory_creation(Path::new("anything")).is_ok());
        assert!(ok_path_removal(Path::new("anything")).is_ok());
        assert!(ok_symlink_creation(Path::new("source"), Path::new("target")).is_ok());
    }

    #[test]
    fn append_orig_suffix_to_backup_paths() {
        assert_eq!(
            backup_path_for_target(std::path::Path::new("/home/.gitconfig")),
            PathBuf::from("/home/.gitconfig.orig")
        );
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
        let directory_creator = FnDirectoryCreator(ok_directory_creation);
        let path_remover = FnPathRemover(ok_path_removal);
        let context = ApplyContext {
            target_probe: &target_probe,
            symlink_creator: &symlink_creator,
            directory_creator: &directory_creator,
            path_remover: &path_remover,
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
        let directory_creator = FnDirectoryCreator(ok_directory_creation);
        let path_remover = FnPathRemover(ok_path_removal);
        let context = ApplyContext {
            target_probe: &target_probe,
            symlink_creator: &symlink_creator,
            directory_creator: &directory_creator,
            path_remover: &path_remover,
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
    fn create_a_directory_symlink_when_the_target_is_missing() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source_directory = temp.path().join("source-dir");
        let target = temp.path().join("target-dir");
        let target_probe = NativeTargetProbe;

        fs::create_dir_all(source_directory.join("nested"))
            .expect("source directory should be created");

        let outcome = apply_symlink_operation(
            &FileOperation {
                source: source_directory.clone(),
                target: target.clone(),
                mkdir: false,
                conflict_policy: ConflictPolicy::Skip,
            },
            &ApplyContext {
                target_probe: &target_probe,
                symlink_creator: &FnSymlinkCreator(super::create_symlink_on_filesystem),
                directory_creator: &FnDirectoryCreator(ok_directory_creation),
                path_remover: &FnPathRemover(ok_path_removal),
            },
        )
        .expect("apply should create a directory symlink when target is missing");

        assert_eq!(
            outcome,
            ApplyOperationOutcome::Applied(OperationAction::Symlink(FileOperation {
                source: source_directory.clone(),
                target: target.clone(),
                mkdir: false,
                conflict_policy: ConflictPolicy::Skip,
            }))
        );
        assert_eq!(
            fs::read_link(&target).expect("directory symlink target should be readable"),
            source_directory
        );
    }

    #[test]
    fn skip_a_directory_symlink_when_the_target_exists() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source_directory = temp.path().join("source-dir");
        let target = temp.path().join("target-dir");
        let target_probe = NativeTargetProbe;

        fs::create_dir_all(source_directory.join("nested"))
            .expect("source directory should be created");
        fs::create_dir_all(target.join("existing"))
            .expect("existing target directory should be created");

        let outcome = apply_symlink_operation(
            &FileOperation {
                source: source_directory,
                target: target.clone(),
                mkdir: false,
                conflict_policy: ConflictPolicy::Skip,
            },
            &ApplyContext {
                target_probe: &target_probe,
                symlink_creator: &FnSymlinkCreator(super::create_symlink_on_filesystem),
                directory_creator: &FnDirectoryCreator(ok_directory_creation),
                path_remover: &FnPathRemover(ok_path_removal),
            },
        )
        .expect("apply should skip existing targets when conflict policy is skip");

        assert_eq!(
            outcome,
            ApplyOperationOutcome::SkippedExistingTarget(OperationAction::Symlink(FileOperation {
                source: temp.path().join("source-dir"),
                target: target.clone(),
                mkdir: false,
                conflict_policy: ConflictPolicy::Skip,
            }))
        );
        assert!(
            target.is_dir(),
            "existing directory target should be preserved"
        );
    }

    #[test]
    fn backup_and_replace_existing_directories_for_directory_symlink_actions() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source_directory = temp.path().join("source-dir");
        let target = temp.path().join("target-dir");
        let backup_target = PathBuf::from(format!("{}.orig", target.display()));
        let target_probe = NativeTargetProbe;

        fs::create_dir_all(source_directory.join("nested"))
            .expect("source directory should be created");
        fs::create_dir_all(&target).expect("existing target directory should be created");
        fs::write(target.join("old.txt"), "old\n").expect("existing target file should be written");

        let outcome = apply_symlink_operation(
            &FileOperation {
                source: source_directory.clone(),
                target: target.clone(),
                mkdir: false,
                conflict_policy: ConflictPolicy::OverwriteWithBackup,
            },
            &ApplyContext {
                target_probe: &target_probe,
                symlink_creator: &FnSymlinkCreator(super::create_symlink_on_filesystem),
                directory_creator: &FnDirectoryCreator(ok_directory_creation),
                path_remover: &FnPathRemover(ok_path_removal),
            },
        )
        .expect("apply should backup and replace existing directory targets");

        assert_eq!(
            outcome,
            ApplyOperationOutcome::Applied(OperationAction::Symlink(FileOperation {
                source: source_directory.clone(),
                target: target.clone(),
                mkdir: false,
                conflict_policy: ConflictPolicy::OverwriteWithBackup,
            }))
        );
        assert_eq!(
            fs::read_link(&target).expect("replacement directory symlink should be readable"),
            source_directory
        );
        assert!(
            backup_target.is_dir(),
            "backup directory should preserve replaced target"
        );
        assert_eq!(
            fs::read_to_string(backup_target.join("old.txt"))
                .expect("backup directory contents should be readable"),
            "old\n"
        );
    }

    #[test]
    fn overwrite_existing_file_targets_for_symlink_actions() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source = temp.path().join("source.txt");
        let target = temp.path().join("target.txt");
        let boundary = RealBoundary;
        fs::write(&source, "source\n").expect("source file should be written");
        fs::write(&target, "target\n").expect("target file should be written");

        let outcome = apply_symlink_operation(
            &FileOperation {
                source: source.clone(),
                target: target.clone(),
                mkdir: false,
                conflict_policy: ConflictPolicy::Overwrite,
            },
            &ApplyContext {
                target_probe: &boundary,
                symlink_creator: &boundary,
                directory_creator: &boundary,
                path_remover: &boundary,
            },
        )
        .expect("existing file targets should be replaced when overwrite is requested");

        assert_eq!(
            outcome,
            ApplyOperationOutcome::Applied(OperationAction::Symlink(FileOperation {
                source: source.clone(),
                target: target.clone(),
                mkdir: false,
                conflict_policy: ConflictPolicy::Overwrite,
            }))
        );
        assert_eq!(
            fs::read_link(&target).expect("symlink target should be readable"),
            source
        );
    }

    #[test]
    fn backup_and_replace_existing_file_targets_for_symlink_actions() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source = temp.path().join("source.txt");
        let target = temp.path().join("target.txt");
        let backup_target = PathBuf::from(format!("{}.orig", target.display()));
        let boundary = RealBoundary;
        fs::write(&source, "source\n").expect("source file should be written");
        fs::write(&target, "target\n").expect("target file should be written");

        let outcome = apply_symlink_operation(
            &FileOperation {
                source: source.clone(),
                target: target.clone(),
                mkdir: false,
                conflict_policy: ConflictPolicy::OverwriteWithBackup,
            },
            &ApplyContext {
                target_probe: &boundary,
                symlink_creator: &boundary,
                directory_creator: &boundary,
                path_remover: &boundary,
            },
        )
        .expect("existing file targets should be backed up and replaced when requested");

        assert_eq!(
            outcome,
            ApplyOperationOutcome::Applied(OperationAction::Symlink(FileOperation {
                source: source.clone(),
                target: target.clone(),
                mkdir: false,
                conflict_policy: ConflictPolicy::OverwriteWithBackup,
            }))
        );
        assert_eq!(
            fs::read_link(&target).expect("symlink target should be readable"),
            source
        );
        assert_eq!(
            fs::read_to_string(&backup_target).expect("backup target should be readable"),
            "target\n"
        );
    }

    #[test]
    fn create_copy_when_target_is_missing() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source = temp.path().join("source.txt");
        let target = temp.path().join("target.txt");
        let boundary = RealBoundary;
        fs::write(&source, "source\n").expect("source file should be written");

        let outcome = apply_copy_operation(
            &FileOperation {
                source: source.clone(),
                target: target.clone(),
                mkdir: false,
                conflict_policy: ConflictPolicy::Skip,
            },
            &ApplyContext {
                target_probe: &boundary,
                symlink_creator: &boundary,
                directory_creator: &boundary,
                path_remover: &boundary,
            },
        )
        .expect("missing copy targets should be created");

        assert_eq!(
            outcome,
            ApplyOperationOutcome::Applied(OperationAction::Copy(FileOperation {
                source,
                target: target.clone(),
                mkdir: false,
                conflict_policy: ConflictPolicy::Skip,
            }))
        );
        assert_eq!(
            fs::read_to_string(&target).expect("copied target should be readable"),
            "source\n"
        );
    }

    #[test]
    fn skip_copy_when_target_exists_and_conflict_policy_is_skip() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source = temp.path().join("source.txt");
        let target = temp.path().join("target.txt");
        let boundary = RealBoundary;
        fs::write(&source, "source\n").expect("source file should be written");
        fs::write(&target, "target\n").expect("target file should be written");

        let outcome = apply_copy_operation(
            &FileOperation {
                source: source.clone(),
                target: target.clone(),
                mkdir: false,
                conflict_policy: ConflictPolicy::Skip,
            },
            &ApplyContext {
                target_probe: &boundary,
                symlink_creator: &boundary,
                directory_creator: &boundary,
                path_remover: &boundary,
            },
        )
        .expect("existing copy targets should be skipped when overwrite is disabled");

        assert_eq!(
            outcome,
            ApplyOperationOutcome::SkippedExistingTarget(OperationAction::Copy(FileOperation {
                source,
                target: target.clone(),
                mkdir: false,
                conflict_policy: ConflictPolicy::Skip,
            }))
        );
        assert_eq!(
            fs::read_to_string(&target).expect("original target should remain readable"),
            "target\n"
        );
    }

    #[test]
    fn backup_and_replace_existing_copy_targets() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source = temp.path().join("source.txt");
        let target = temp.path().join("target.txt");
        let backup_target = PathBuf::from(format!("{}.orig", target.display()));
        let boundary = RealBoundary;
        fs::write(&source, "source\n").expect("source file should be written");
        fs::write(&target, "target\n").expect("target file should be written");

        let outcome = apply_copy_operation(
            &FileOperation {
                source: source.clone(),
                target: target.clone(),
                mkdir: false,
                conflict_policy: ConflictPolicy::OverwriteWithBackup,
            },
            &ApplyContext {
                target_probe: &boundary,
                symlink_creator: &boundary,
                directory_creator: &boundary,
                path_remover: &boundary,
            },
        )
        .expect("existing copy targets should be backed up and replaced when requested");

        assert_eq!(
            outcome,
            ApplyOperationOutcome::Applied(OperationAction::Copy(FileOperation {
                source,
                target: target.clone(),
                mkdir: false,
                conflict_policy: ConflictPolicy::OverwriteWithBackup,
            }))
        );
        assert_eq!(
            fs::read_to_string(&target).expect("replaced target should be readable"),
            "source\n"
        );
        assert_eq!(
            fs::read_to_string(&backup_target).expect("backup target should be readable"),
            "target\n"
        );
    }

    #[test]
    fn skip_copy_overwrite_when_target_contents_already_match() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source = temp.path().join("source.txt");
        let target = temp.path().join("target.txt");
        let target_probe = NativeTargetProbe;
        let symlink_creator = FnSymlinkCreator(ok_symlink_creation);
        let directory_creator = FnDirectoryCreator(ok_directory_creation);
        let path_remover = FnPathRemover(ok_path_removal);
        fs::write(&source, "same\n").expect("source file should be written");
        fs::write(&target, "same\n").expect("target file should be written");

        let outcome = apply_copy_operation(
            &FileOperation {
                source: source.clone(),
                target: target.clone(),
                mkdir: true,
                conflict_policy: ConflictPolicy::Overwrite,
            },
            &ApplyContext {
                target_probe: &target_probe,
                symlink_creator: &symlink_creator,
                directory_creator: &directory_creator,
                path_remover: &path_remover,
            },
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
        let symlink_creator = FnSymlinkCreator(ok_symlink_creation);
        let directory_creator = FnDirectoryCreator(ok_directory_creation);
        let path_remover = FnPathRemover(ok_path_removal);
        fs::write(&source, "source\n").expect("source file should be written");
        fs::write(&target, "target\n").expect("target file should be written");

        let outcome = apply_copy_operation(
            &FileOperation {
                source: source.clone(),
                target: target.clone(),
                mkdir: true,
                conflict_policy: ConflictPolicy::Overwrite,
            },
            &ApplyContext {
                target_probe: &target_probe,
                symlink_creator: &symlink_creator,
                directory_creator: &directory_creator,
                path_remover: &path_remover,
            },
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
            entries: vec![OperationEntry::Action(PlannedOperationAction {
                action: OperationAction::Copy(FileOperation {
                    source: PathBuf::from("/repo/source"),
                    target: PathBuf::from("/home/target"),
                    mkdir: true,
                    conflict_policy: ConflictPolicy::Skip,
                }),
                source_availability: SourceAvailability::Available,
                skip_reason: None,
            })],
        };
        let target_probe = FnTargetProbe(|_| Ok(false));
        let symlink_creator = FnSymlinkCreator(ok_symlink_creation);
        let directory_creator = FnDirectoryCreator(ok_directory_creation);
        let path_remover = FnPathRemover(ok_path_removal);
        let context = ApplyContext {
            target_probe: &target_probe,
            symlink_creator: &symlink_creator,
            directory_creator: &directory_creator,
            path_remover: &path_remover,
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

    #[test]
    fn cannot_apply_when_any_planned_source_is_unavailable() {
        let operation_plan = OperationPlan {
            entries: vec![OperationEntry::Action(PlannedOperationAction {
                action: OperationAction::Symlink(FileOperation {
                    source: PathBuf::from("/repo/private/.secret"),
                    target: PathBuf::from("/home/.secret"),
                    mkdir: true,
                    conflict_policy: ConflictPolicy::Skip,
                }),
                source_availability: SourceAvailability::Unreadable {
                    kind: crate::SourceUnreadableKind::PermissionDenied,
                    message: "permission denied".to_string(),
                },
                skip_reason: None,
            })],
        };
        let target_probe = FnTargetProbe(|_| Ok(false));
        let symlink_creator = FnSymlinkCreator(ok_symlink_creation);
        let directory_creator = FnDirectoryCreator(ok_directory_creation);
        let path_remover = FnPathRemover(ok_path_removal);
        let context = ApplyContext {
            target_probe: &target_probe,
            symlink_creator: &symlink_creator,
            directory_creator: &directory_creator,
            path_remover: &path_remover,
        };

        let error = apply_operation_plan(&operation_plan, &context)
            .expect_err("apply should stop before mutation when a source is unavailable");

        assert_eq!(
            error,
            ProgramError::OperationPlanBlocked {
                diagnostics: vec![
                    "source /repo/private/.secret for /home/.secret is unreadable (permission denied)"
                        .to_string(),
                ],
            }
        );
    }

    #[test]
    fn skip_apply_work_when_a_planned_action_requires_a_missing_target_directory() {
        let operation_plan = OperationPlan {
            entries: vec![OperationEntry::Action(PlannedOperationAction {
                action: OperationAction::Symlink(FileOperation {
                    source: PathBuf::from("/repo/nested/tool.conf"),
                    target: PathBuf::from("/home/profiles/nested/tool.conf"),
                    mkdir: true,
                    conflict_policy: ConflictPolicy::Skip,
                }),
                source_availability: SourceAvailability::Available,
                skip_reason: Some(crate::operation::OperationSkipReason::MissingTargetDirectory),
            })],
        };
        let target_probe = FnTargetProbe(|_| Ok(false));
        let symlink_creator = FnSymlinkCreator(super::create_symlink_on_filesystem);
        let directory_creator = FnDirectoryCreator(ok_directory_creation);
        let path_remover = FnPathRemover(ok_path_removal);
        let context = ApplyContext {
            target_probe: &target_probe,
            symlink_creator: &symlink_creator,
            directory_creator: &directory_creator,
            path_remover: &path_remover,
        };

        let report = apply_operation_plan(&operation_plan, &context)
            .expect("apply should skip planned actions with missing target directories");

        assert_eq!(
            report,
            crate::ApplyOperationReport {
                outcomes: vec![ApplyOperationOutcome::SkippedMissingTargetDirectory(
                    OperationAction::Symlink(FileOperation {
                        source: PathBuf::from("/repo/nested/tool.conf"),
                        target: PathBuf::from("/home/profiles/nested/tool.conf"),
                        mkdir: true,
                        conflict_policy: ConflictPolicy::Skip,
                    })
                )],
            }
        );
    }

    #[test]
    fn cannot_apply_when_a_planned_source_root_is_missing() {
        let operation_plan = OperationPlan {
            entries: vec![OperationEntry::Issue(crate::OperationPlanningIssue {
                path: PathBuf::from("/repo/profiles"),
                source_availability: SourceAvailability::Missing,
            })],
        };
        let target_probe = FnTargetProbe(|_| Ok(false));
        let symlink_creator = FnSymlinkCreator(ok_symlink_creation);
        let directory_creator = FnDirectoryCreator(ok_directory_creation);
        let path_remover = FnPathRemover(ok_path_removal);
        let context = ApplyContext {
            target_probe: &target_probe,
            symlink_creator: &symlink_creator,
            directory_creator: &directory_creator,
            path_remover: &path_remover,
        };

        let error = apply_operation_plan(&operation_plan, &context)
            .expect_err("apply should stop before mutation when a source root is missing");

        assert_eq!(
            error,
            ProgramError::OperationPlanBlocked {
                diagnostics: vec!["source root /repo/profiles is missing".to_string()],
            }
        );
    }

    #[test]
    fn skip_non_action_entries_while_applying_action_entries() {
        let operation_plan = OperationPlan {
            entries: vec![
                OperationEntry::Issue(crate::OperationPlanningIssue {
                    path: PathBuf::from("/repo/ignored"),
                    source_availability: SourceAvailability::Available,
                }),
                OperationEntry::Action(PlannedOperationAction {
                    action: OperationAction::Symlink(FileOperation {
                        source: PathBuf::from("/repo/.zshrc"),
                        target: PathBuf::from("/home/.zshrc"),
                        mkdir: false,
                        conflict_policy: ConflictPolicy::Skip,
                    }),
                    source_availability: SourceAvailability::Available,
                    skip_reason: None,
                }),
            ],
        };
        let target_probe = FnTargetProbe(|_| Ok(false));
        let symlink_creator = FnSymlinkCreator(|_, _| Ok(()));
        let directory_creator = FnDirectoryCreator(|_| Ok(()));
        let path_remover = FnPathRemover(|_| Ok(()));
        let context = ApplyContext {
            target_probe: &target_probe,
            symlink_creator: &symlink_creator,
            directory_creator: &directory_creator,
            path_remover: &path_remover,
        };

        let report = apply_operation_plan(&operation_plan, &context)
            .expect("apply should skip non-action entries and process concrete actions");

        assert_eq!(
            report,
            crate::ApplyOperationReport {
                outcomes: vec![ApplyOperationOutcome::Applied(OperationAction::Symlink(
                    FileOperation {
                        source: PathBuf::from("/repo/.zshrc"),
                        target: PathBuf::from("/home/.zshrc"),
                        mkdir: false,
                        conflict_policy: ConflictPolicy::Skip,
                    }
                ))],
            }
        );
    }

    #[test]
    fn cannot_apply_symlink_when_symlink_creator_fails() {
        let operation_plan = symlink_plan(
            PathBuf::from("/repo/source.txt"),
            PathBuf::from("/home/target.txt"),
            false,
            ConflictPolicy::Skip,
        );
        let target_probe = FnTargetProbe(|_| Ok(false));
        let symlink_creator = FnSymlinkCreator(|_, _| {
            Err(ProgramError::SymlinkCreationFailed {
                source: PathBuf::from("/repo/source.txt"),
                target: PathBuf::from("/home/target.txt"),
                message: "permission denied".to_string(),
            })
        });
        let directory_creator = FnDirectoryCreator(ok_directory_creation);
        let path_remover = FnPathRemover(ok_path_removal);
        let context = ApplyContext {
            target_probe: &target_probe,
            symlink_creator: &symlink_creator,
            directory_creator: &directory_creator,
            path_remover: &path_remover,
        };

        let error = apply_operation_plan(&operation_plan, &context)
            .expect_err("apply should propagate symlink creation failures");

        assert!(matches!(&error, ProgramError::SymlinkCreationFailed { .. }));
    }

    #[test]
    fn cannot_apply_symlink_when_directory_creator_fails() {
        let operation_plan = symlink_plan(
            PathBuf::from("/repo/source.txt"),
            PathBuf::from("/home/nested/target.txt"),
            true,
            ConflictPolicy::Skip,
        );
        let target_probe = FnTargetProbe(|_| Ok(false));
        let symlink_creator = FnSymlinkCreator(super::create_symlink_on_filesystem);
        let directory_creator = FnDirectoryCreator(|path| {
            Err(ProgramError::TargetDirectoryCreationFailed {
                path: path.to_path_buf(),
                message: "permission denied".to_string(),
            })
        });
        let path_remover = FnPathRemover(ok_path_removal);
        let context = ApplyContext {
            target_probe: &target_probe,
            symlink_creator: &symlink_creator,
            directory_creator: &directory_creator,
            path_remover: &path_remover,
        };

        let error = apply_operation_plan(&operation_plan, &context)
            .expect_err("apply should propagate directory creation failures");

        assert!(matches!(
            &error,
            ProgramError::TargetDirectoryCreationFailed { .. }
        ));
    }

    #[test]
    fn cannot_apply_symlink_overwrite_when_path_remover_fails() {
        let operation_plan = symlink_plan(
            PathBuf::from("/repo/source.txt"),
            PathBuf::from("/home/target.txt"),
            false,
            ConflictPolicy::Overwrite,
        );
        let target_probe = FnTargetProbe(|_| Ok(true));
        let symlink_creator = FnSymlinkCreator(super::create_symlink_on_filesystem);
        let directory_creator = FnDirectoryCreator(ok_directory_creation);
        let path_remover = FnPathRemover(|path| {
            Err(ProgramError::TargetRemovalFailed {
                path: path.to_path_buf(),
                message: "permission denied".to_string(),
            })
        });
        let context = ApplyContext {
            target_probe: &target_probe,
            symlink_creator: &symlink_creator,
            directory_creator: &directory_creator,
            path_remover: &path_remover,
        };

        let error = apply_operation_plan(&operation_plan, &context)
            .expect_err("apply should propagate path removal failures");

        assert!(matches!(&error, ProgramError::TargetRemovalFailed { .. }));
    }

    #[test]
    fn cannot_apply_copy_when_directory_creator_fails() {
        let operation_plan = copy_plan(
            PathBuf::from("/repo/source.txt"),
            PathBuf::from("/home/nested/target.txt"),
            true,
            ConflictPolicy::Skip,
        );
        let target_probe = FnTargetProbe(|_| Ok(false));
        let symlink_creator = FnSymlinkCreator(|_, _| Ok(()));
        let directory_creator = FnDirectoryCreator(|path| {
            Err(ProgramError::TargetDirectoryCreationFailed {
                path: path.to_path_buf(),
                message: "permission denied".to_string(),
            })
        });
        let path_remover = FnPathRemover(ok_path_removal);
        let context = ApplyContext {
            target_probe: &target_probe,
            symlink_creator: &symlink_creator,
            directory_creator: &directory_creator,
            path_remover: &path_remover,
        };

        let error = apply_operation_plan(&operation_plan, &context)
            .expect_err("apply should propagate directory creation failures for copy");

        assert!(matches!(
            &error,
            ProgramError::TargetDirectoryCreationFailed { .. }
        ));
    }
}
