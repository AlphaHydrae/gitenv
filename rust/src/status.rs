use crate::{FileOperation, OperationAction, OperationPlan, ProgramError, logging};
use log::Level;
use std::hash::Hasher;
use std::io::Read;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetKind {
    Symlink,
    Missing,
    NotASymlink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CopyTargetKind {
    File,
    Missing,
    NotAFile,
}

/// Structured status for one operation-plan inspection pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationInspectionReport {
    pub outcomes: Vec<OperationInspectionOutcome>,
}

/// Per-operation status outcome for symlink and copy operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationInspectionOutcome {
    Symlink(SymlinkInspection),
    Copy(CopyInspection),
}

/// Structured status for a single concrete symlink operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymlinkInspection {
    pub source: PathBuf,
    pub target: PathBuf,
    pub state: SymlinkInspectionState,
}

/// Structured status for a single concrete copy operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyInspection {
    pub source: PathBuf,
    pub target: PathBuf,
    pub state: CopyInspectionState,
}

/// Current status of a concrete symlink target path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymlinkInspectionState {
    Ok,
    Missing,
    NotASymlink,
    PointsElsewhere { current_target: PathBuf },
}

/// Current status of a concrete copy target path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyInspectionState {
    Ok,
    Missing,
    NotAFile,
    Differs,
}

/// Inspects status for every operation in an operation plan without mutating
/// the filesystem.
pub fn inspect_operation_plan_status(
    operation_plan: &OperationPlan,
) -> Result<OperationInspectionReport, ProgramError> {
    logging::status(
        Level::Debug,
        "inspect_operation_plan_status_start",
        format!("actions={}", operation_plan.actions.len()),
    );

    let mut outcomes = Vec::new();

    for action in &operation_plan.actions {
        let outcome = match action {
            OperationAction::Symlink(operation) => {
                OperationInspectionOutcome::Symlink(inspect_symlink_operation_status(operation)?)
            }
            OperationAction::Copy(operation) => {
                OperationInspectionOutcome::Copy(inspect_copy_operation_status(operation)?)
            }
        };
        outcomes.push(outcome);
    }

    logging::status(
        Level::Info,
        "inspect_operation_plan_status_success",
        format!("outcomes={}", outcomes.len()),
    );

    Ok(OperationInspectionReport { outcomes })
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

/// Inspects status for one copy operation without mutating the filesystem.
pub fn inspect_copy_operation_status(
    operation: &FileOperation,
) -> Result<CopyInspection, ProgramError> {
    inspect_copy_operation_status_with_injectables(
        operation,
        &copy_target_kind_from_filesystem,
        &hash_file_contents_from_filesystem,
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

    logging::status(
        Level::Debug,
        "inspect_symlink_operation_status",
        format!("state={state:?}"),
    );

    Ok(SymlinkInspection {
        source: operation.source.clone(),
        target: operation.target.clone(),
        state,
    })
}

fn inspect_copy_operation_status_with_injectables(
    operation: &FileOperation,
    target_kind: &impl Fn(&std::path::Path) -> Result<CopyTargetKind, ProgramError>,
    hash_file_contents: &impl Fn(&std::path::Path) -> Result<u64, ProgramError>,
) -> Result<CopyInspection, ProgramError> {
    let state = match target_kind(&operation.target)? {
        CopyTargetKind::Missing => CopyInspectionState::Missing,
        CopyTargetKind::NotAFile => CopyInspectionState::NotAFile,
        CopyTargetKind::File => {
            let source_hash = hash_file_contents(&operation.source)?;
            let target_hash = hash_file_contents(&operation.target)?;

            if source_hash == target_hash {
                CopyInspectionState::Ok
            } else {
                CopyInspectionState::Differs
            }
        }
    };

    logging::status(
        Level::Debug,
        "inspect_copy_operation_status",
        format!("state={state:?}"),
    );

    Ok(CopyInspection {
        source: operation.source.clone(),
        target: operation.target.clone(),
        state,
    })
}

fn target_kind_from_filesystem(path: &std::path::Path) -> Result<TargetKind, ProgramError> {
    logging::system(
        Level::Trace,
        "symlink_metadata",
        format!("path={}", path.display()),
    );

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
    logging::system(
        Level::Trace,
        "read_link",
        format!("path={}", path.display()),
    );

    std::fs::read_link(path).map_err(|error| ProgramError::ReadSymlinkTarget {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

fn copy_target_kind_from_filesystem(
    path: &std::path::Path,
) -> Result<CopyTargetKind, ProgramError> {
    logging::system(
        Level::Trace,
        "symlink_metadata",
        format!("path={}", path.display()),
    );

    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_file() {
                Ok(CopyTargetKind::File)
            } else {
                Ok(CopyTargetKind::NotAFile)
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(CopyTargetKind::Missing),
        Err(error) => Err(ProgramError::InspectPathMetadata {
            path: path.to_path_buf(),
            message: error.to_string(),
        }),
    }
}

fn hash_file_contents_from_filesystem(path: &std::path::Path) -> Result<u64, ProgramError> {
    logging::system(
        Level::Trace,
        "hash_file_start",
        format!("path={}", path.display()),
    );

    let file = std::fs::File::open(path).map_err(|error| ProgramError::InspectPathMetadata {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut reader = std::io::BufReader::new(file);
    let mut buffer = [0_u8; 8192];
    let mut hasher = std::collections::hash_map::DefaultHasher::new();

    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| ProgramError::InspectPathMetadata {
                path: path.to_path_buf(),
                message: error.to_string(),
            })?;

        if read == 0 {
            break;
        }

        hasher.write(&buffer[..read]);
    }

    let digest = hasher.finish();

    logging::system(
        Level::Trace,
        "hash_file_success",
        format!("path={} digest={digest:#018x}", path.display()),
    );

    Ok(digest)
}

#[cfg(test)]
mod tests {
    use super::{
        CopyInspection, CopyInspectionState, CopyTargetKind, OperationInspectionOutcome,
        OperationInspectionReport, SymlinkInspection, SymlinkInspectionState, TargetKind,
        copy_target_kind_from_filesystem, hash_file_contents_from_filesystem,
        inspect_copy_operation_status, inspect_copy_operation_status_with_injectables,
        inspect_operation_plan_status, inspect_symlink_operation_status,
        inspect_symlink_operation_status_with_injectables, read_symlink_target_from_filesystem,
        target_kind_from_filesystem,
    };
    use crate::{ConflictPolicy, FileOperation, OperationAction, OperationPlan, ProgramError};
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
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

    fn make_operation_plan(actions: Vec<OperationAction>) -> OperationPlan {
        OperationPlan { actions }
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
            &read_symlink_target_from_filesystem,
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

    #[test]
    fn report_ok_when_copy_target_matches_the_expected_source() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source = temp.path().join("source.txt");
        let target = temp.path().join("target.txt");

        fs::write(&source, "content\n").expect("source file should be written");
        fs::write(&target, "content\n").expect("target file should be written");

        let operation = make_operation(source.clone(), target.clone());
        let status =
            inspect_copy_operation_status(&operation).expect("copy status inspection should work");

        assert_eq!(
            status,
            CopyInspection {
                source,
                target,
                state: CopyInspectionState::Ok,
            }
        );
    }

    #[test]
    fn report_missing_when_copy_target_is_absent() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source = temp.path().join("source.txt");
        let target = temp.path().join("target.txt");
        fs::write(&source, "content\n").expect("source file should be written");

        let operation = make_operation(source.clone(), target.clone());
        let status =
            inspect_copy_operation_status(&operation).expect("copy status inspection should work");

        assert_eq!(
            status,
            CopyInspection {
                source,
                target,
                state: CopyInspectionState::Missing,
            }
        );
    }

    #[test]
    fn report_not_a_file_when_copy_target_path_is_a_directory() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source = temp.path().join("source.txt");
        let target = temp.path().join("target-dir");
        fs::write(&source, "content\n").expect("source file should be written");
        fs::create_dir_all(&target).expect("target directory should be created");

        let operation = make_operation(source.clone(), target.clone());
        let status =
            inspect_copy_operation_status(&operation).expect("copy status inspection should work");

        assert_eq!(
            status,
            CopyInspection {
                source,
                target,
                state: CopyInspectionState::NotAFile,
            }
        );
    }

    #[test]
    fn report_differs_when_copy_target_contents_do_not_match_source() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source = temp.path().join("source.txt");
        let target = temp.path().join("target.txt");
        fs::write(&source, "source\n").expect("source file should be written");
        fs::write(&target, "target\n").expect("target file should be written");

        let operation = make_operation(source.clone(), target.clone());
        let status =
            inspect_copy_operation_status(&operation).expect("copy status inspection should work");

        assert_eq!(
            status,
            CopyInspection {
                source,
                target,
                state: CopyInspectionState::Differs,
            }
        );
    }

    #[test]
    fn inspect_both_symlink_and_copy_operations_in_a_single_operation_plan() {
        let operation_plan = make_operation_plan(vec![
            OperationAction::Symlink(make_operation(
                PathBuf::from("/repo/.gitconfig"),
                PathBuf::from("/home/.gitconfig"),
            )),
            OperationAction::Copy(make_operation(
                PathBuf::from("/repo/.zshrc"),
                PathBuf::from("/home/.zshrc"),
            )),
        ]);

        let report =
            inspect_operation_plan_status(&operation_plan).expect("status inspection should work");

        assert_eq!(
            report,
            OperationInspectionReport {
                outcomes: vec![
                    OperationInspectionOutcome::Symlink(SymlinkInspection {
                        source: PathBuf::from("/repo/.gitconfig"),
                        target: PathBuf::from("/home/.gitconfig"),
                        state: SymlinkInspectionState::Missing,
                    }),
                    OperationInspectionOutcome::Copy(CopyInspection {
                        source: PathBuf::from("/repo/.zshrc"),
                        target: PathBuf::from("/home/.zshrc"),
                        state: CopyInspectionState::Missing,
                    }),
                ],
            }
        );
    }

    #[test]
    fn propagate_copy_read_errors_while_inspecting_copy_status() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source = temp.path().join("source.txt");
        let target = temp.path().join("target.txt");
        let operation = make_operation(source.clone(), target.clone());

        let error = inspect_copy_operation_status_with_injectables(
            &operation,
            &|_| Ok(CopyTargetKind::File),
            &|path| {
                Err(ProgramError::InspectPathMetadata {
                    path: path.to_path_buf(),
                    message: "permission denied".to_string(),
                })
            },
        )
        .expect_err("copy read failures should be propagated");

        assert_eq!(
            error,
            ProgramError::InspectPathMetadata {
                path: source,
                message: "permission denied".to_string(),
            }
        );
    }

    #[test]
    fn report_hash_open_errors_for_missing_paths() {
        let missing = PathBuf::from("/path/that/does/not/exist");

        let error = hash_file_contents_from_filesystem(&missing)
            .expect_err("missing files should fail during hash open");

        assert!(matches!(
            &error,
            ProgramError::InspectPathMetadata { path, message }
                if path == &missing && !message.is_empty()
        ));
    }

    #[test]
    fn report_read_symlink_target_errors_for_missing_paths() {
        let missing = PathBuf::from("/path/that/does/not/exist");

        let error = read_symlink_target_from_filesystem(&missing)
            .expect_err("missing paths should fail during symlink target reads");

        assert!(matches!(
            &error,
            ProgramError::ReadSymlinkTarget { path, message }
                if path == &missing && !message.is_empty()
        ));
    }

    #[cfg(unix)]
    #[test]
    fn report_symlink_kind_metadata_errors_when_parent_directory_is_not_accessible() {
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

        let error = target_kind_from_filesystem(&target)
            .expect_err("metadata probes should fail for inaccessible directory entries");

        let mut restore_permissions = fs::metadata(&locked_directory)
            .expect("locked directory metadata should be readable")
            .permissions();
        restore_permissions.set_mode(0o700);
        fs::set_permissions(&locked_directory, restore_permissions)
            .expect("locked directory permissions should be restored");

        assert!(matches!(
            &error,
            ProgramError::InspectPathMetadata { path, message }
                if path == &target && !message.is_empty()
        ));
    }

    #[cfg(unix)]
    #[test]
    fn report_copy_kind_metadata_errors_when_parent_directory_is_not_accessible() {
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

        let error = copy_target_kind_from_filesystem(&target)
            .expect_err("metadata probes should fail for inaccessible directory entries");

        let mut restore_permissions = fs::metadata(&locked_directory)
            .expect("locked directory metadata should be readable")
            .permissions();
        restore_permissions.set_mode(0o700);
        fs::set_permissions(&locked_directory, restore_permissions)
            .expect("locked directory permissions should be restored");

        assert!(matches!(
            &error,
            ProgramError::InspectPathMetadata { path, message }
                if path == &target && !message.is_empty()
        ));
    }

    #[cfg(unix)]
    #[test]
    fn report_hash_read_errors_for_directory_paths() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let directory = temp.path().join("a-directory");
        fs::create_dir_all(&directory).expect("directory should be created");

        let error = hash_file_contents_from_filesystem(&directory)
            .expect_err("directory paths should fail during hash read");

        assert!(matches!(
            &error,
            ProgramError::InspectPathMetadata { path, message }
                if path == &directory && !message.is_empty()
        ));
    }
}
