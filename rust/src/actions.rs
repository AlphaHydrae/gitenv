use crate::{FileOperation, OperationAction, OperationPlan, ProgramError};
use std::path::Path;

/// Structured apply outcomes for one operation-plan execution pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyOperationReport {
    pub outcomes: Vec<ApplyOperationOutcome>,
}

/// Per-operation apply result for incremental symlink execution support.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyOperationOutcome {
    AppliedSymlink(FileOperation),
    SkippedExistingTarget(FileOperation),
    UnsupportedOperation(OperationAction),
}

/// Applies operation-plan actions using real filesystem access.
///
/// This increment intentionally supports only the simplest symlink path:
/// when the target path does not exist.
pub fn apply_operation_plan(
    operation_plan: &OperationPlan,
) -> Result<ApplyOperationReport, ProgramError> {
    apply_operation_plan_with_injectables(
        operation_plan,
        &target_exists,
        &create_symlink_on_filesystem,
    )
}

/// Like `apply_operation_plan` but accepts injectable filesystem probes for
/// deterministic tests.
pub fn apply_operation_plan_with_injectables(
    operation_plan: &OperationPlan,
    target_exists: &impl Fn(&Path) -> Result<bool, ProgramError>,
    create_symlink: &impl Fn(&Path, &Path) -> Result<(), ProgramError>,
) -> Result<ApplyOperationReport, ProgramError> {
    let mut outcomes = Vec::new();

    for action in &operation_plan.actions {
        match action {
            OperationAction::Symlink(operation) => {
                if target_exists(&operation.target)? {
                    outcomes.push(ApplyOperationOutcome::SkippedExistingTarget(
                        operation.clone(),
                    ));
                } else {
                    create_symlink(&operation.source, &operation.target)?;
                    outcomes.push(ApplyOperationOutcome::AppliedSymlink(operation.clone()));
                }
            }
            unsupported => {
                outcomes.push(ApplyOperationOutcome::UnsupportedOperation(
                    unsupported.clone(),
                ));
            }
        }
    }

    Ok(ApplyOperationReport { outcomes })
}

fn target_exists(path: &Path) -> Result<bool, ProgramError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(ProgramError::InspectPathMetadata {
            path: path.to_path_buf(),
            message: error.to_string(),
        }),
    }
}

#[cfg(unix)]
fn create_symlink_on_filesystem(source: &Path, target: &Path) -> Result<(), ProgramError> {
    std::os::unix::fs::symlink(source, target).map_err(|error| ProgramError::CreateSymlink {
        source: source.to_path_buf(),
        target: target.to_path_buf(),
        message: error.to_string(),
    })
}

#[cfg(not(unix))]
fn create_symlink_on_filesystem(source: &Path, target: &Path) -> Result<(), ProgramError> {
    Err(ProgramError::CreateSymlink {
        source: source.to_path_buf(),
        target: target.to_path_buf(),
        message: "symlink apply is not supported on this platform".to_string(),
    })
}
