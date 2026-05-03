use crate::{FileOperation, ProgramError};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetKind {
    Symlink,
    Missing,
    NotASymlink,
}

/// Structured status for a single concrete symlink operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymlinkInspection {
    pub source: PathBuf,
    pub target: PathBuf,
    pub state: SymlinkInspectionState,
}

/// Current status of a concrete symlink target path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymlinkInspectionState {
    Ok,
    Missing,
    NotASymlink,
    PointsElsewhere { current_target: PathBuf },
}

/// Inspects status for one symlink operation without mutating the filesystem.
pub fn inspect_symlink_operation_status(
    operation: &FileOperation,
) -> Result<SymlinkInspection, ProgramError> {
    inspect_symlink_operation_status_with_injectables(
        operation,
        &target_kind_from_filesystem,
        &read_symlink_target_from_filesystem,
    )
}

fn inspect_symlink_operation_status_with_injectables(
    operation: &FileOperation,
    target_kind: &impl Fn(&std::path::Path) -> Result<TargetKind, ProgramError>,
    read_symlink_target: &impl Fn(&std::path::Path) -> Result<PathBuf, ProgramError>,
) -> Result<SymlinkInspection, ProgramError> {
    let state = match target_kind(&operation.target)? {
        TargetKind::Symlink => {
            let current_target = read_symlink_target(&operation.target)?;
            if current_target == operation.source {
                SymlinkInspectionState::Ok
            } else {
                SymlinkInspectionState::PointsElsewhere { current_target }
            }
        }
        TargetKind::Missing => SymlinkInspectionState::Missing,
        TargetKind::NotASymlink => SymlinkInspectionState::NotASymlink,
    };

    Ok(SymlinkInspection {
        source: operation.source.clone(),
        target: operation.target.clone(),
        state,
    })
}

fn target_kind_from_filesystem(path: &std::path::Path) -> Result<TargetKind, ProgramError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                Ok(TargetKind::Symlink)
            } else {
                Ok(TargetKind::NotASymlink)
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(TargetKind::Missing),
        Err(error) => Err(ProgramError::InspectPathMetadata {
            path: path.to_path_buf(),
            message: error.to_string(),
        }),
    }
}

fn read_symlink_target_from_filesystem(path: &std::path::Path) -> Result<PathBuf, ProgramError> {
    std::fs::read_link(path).map_err(|error| ProgramError::ReadSymlinkTarget {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        SymlinkInspection, SymlinkInspectionState, TargetKind, inspect_symlink_operation_status,
        inspect_symlink_operation_status_with_injectables,
    };
    use crate::{ConflictPolicy, FileOperation, ProgramError};
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn make_operation(source: std::path::PathBuf, target: std::path::PathBuf) -> FileOperation {
        FileOperation {
            source,
            target,
            mkdir: true,
            conflict_policy: ConflictPolicy::Skip,
        }
    }

    #[cfg(unix)]
    #[test]
    fn report_ok_when_the_symlink_points_to_the_expected_source() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source = temp.path().join("source.txt");
        let target = temp.path().join("target.txt");

        fs::write(&source, "content\n").expect("source file should be written");
        symlink(&source, &target).expect("target symlink should be created");

        let operation = make_operation(source.clone(), target.clone());

        let status =
            inspect_symlink_operation_status(&operation).expect("status inspection should work");

        assert_eq!(
            status,
            SymlinkInspection {
                source,
                target,
                state: SymlinkInspectionState::Ok,
            }
        );
    }

    #[test]
    fn report_missing_when_the_symlink_target_is_absent() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source = temp.path().join("source.txt");
        let target = temp.path().join("missing.txt");

        let operation = make_operation(source.clone(), target.clone());
        let status =
            inspect_symlink_operation_status(&operation).expect("status inspection should work");

        assert_eq!(
            status,
            SymlinkInspection {
                source,
                target,
                state: SymlinkInspectionState::Missing,
            }
        );
    }

    #[test]
    fn report_not_a_symlink_when_the_target_path_is_a_regular_file() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source = temp.path().join("source.txt");
        let target = temp.path().join("target.txt");

        fs::write(&source, "source\n").expect("source file should be written");
        fs::write(&target, "existing\n").expect("target file should be written");

        let operation = make_operation(source.clone(), target.clone());
        let status =
            inspect_symlink_operation_status(&operation).expect("status inspection should work");

        assert_eq!(
            status,
            SymlinkInspection {
                source,
                target,
                state: SymlinkInspectionState::NotASymlink,
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn report_points_elsewhere_when_the_symlink_target_differs_from_the_source() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source = temp.path().join("source.txt");
        let other = temp.path().join("other.txt");
        let target = temp.path().join("target.txt");

        fs::write(&source, "source\n").expect("source file should be written");
        fs::write(&other, "other\n").expect("alternate file should be written");
        symlink(&other, &target).expect("target symlink should be created");

        let operation = make_operation(source.clone(), target.clone());
        let status =
            inspect_symlink_operation_status(&operation).expect("status inspection should work");

        assert_eq!(
            status,
            SymlinkInspection {
                source,
                target,
                state: SymlinkInspectionState::PointsElsewhere {
                    current_target: other,
                },
            }
        );
    }

    #[test]
    fn propagate_metadata_errors_while_inspecting_symlink_status() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source = temp.path().join("source.txt");
        let target = temp.path().join("target.txt");
        let operation = make_operation(source, target.clone());

        let error = inspect_symlink_operation_status_with_injectables(
            &operation,
            &|path| {
                Err(ProgramError::InspectPathMetadata {
                    path: path.to_path_buf(),
                    message: "permission denied".to_string(),
                })
            },
            &|_| Ok(PathBuf::from("unused")),
        )
        .expect_err("metadata failures should be propagated");

        assert_eq!(
            error,
            ProgramError::InspectPathMetadata {
                path: target,
                message: "permission denied".to_string(),
            }
        );
    }

    #[test]
    fn propagate_readlink_errors_while_inspecting_symlink_status() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source = temp.path().join("source.txt");
        let target = temp.path().join("target.txt");
        let operation = make_operation(source, target.clone());

        let error = inspect_symlink_operation_status_with_injectables(
            &operation,
            &|_| Ok(TargetKind::Symlink),
            &|path| {
                Err(ProgramError::ReadSymlinkTarget {
                    path: path.to_path_buf(),
                    message: "broken link".to_string(),
                })
            },
        )
        .expect_err("readlink failures should be propagated");

        assert_eq!(
            error,
            ProgramError::ReadSymlinkTarget {
                path: target,
                message: "broken link".to_string(),
            }
        );
    }
}
