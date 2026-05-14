use crate::boundary::{SourcePathRequirement, SourceReadError, SourceReadErrorKind};
use crate::{ProgramError, logging};
use log::Level;
use std::fs::File;
use std::path::Path;
use std::path::PathBuf;

pub(crate) fn resolve_home_directory(
    get_env_var: &dyn Fn(&str) -> Option<String>,
) -> Result<PathBuf, ProgramError> {
    get_env_var("HOME")
        .map(PathBuf::from)
        .ok_or(ProgramError::HomeDirectoryUnavailable)
}

fn source_read_error(path: &Path, error: std::io::Error) -> SourceReadError {
    let kind = match error.kind() {
        std::io::ErrorKind::NotFound => SourceReadErrorKind::Missing,
        std::io::ErrorKind::PermissionDenied => SourceReadErrorKind::PermissionDenied,
        _ => SourceReadErrorKind::UnexpectedIo,
    };

    SourceReadError {
        path: path.to_path_buf(),
        kind,
        message: error.to_string(),
    }
}

fn source_shape_error(path: &Path, message: &str) -> SourceReadError {
    SourceReadError {
        path: path.to_path_buf(),
        kind: SourceReadErrorKind::UnexpectedIo,
        message: message.to_string(),
    }
}

pub(crate) fn ensure_source_path_readable(
    path: &Path,
    requirement: SourcePathRequirement,
) -> Result<(), SourceReadError> {
    logging::system(
        Level::Trace,
        "source_readable",
        format!("path={}", path.display()),
    );

    let metadata =
        std::fs::symlink_metadata(path).map_err(|error| source_read_error(path, error))?;

    match requirement {
        SourcePathRequirement::File => {
            if !(metadata.file_type().is_file() || metadata.file_type().is_symlink()) {
                return Err(source_shape_error(path, "not a readable file"));
            }

            File::open(path)
                .map(|_| ())
                .map_err(|error| source_read_error(path, error))
        }
        SourcePathRequirement::Directory => {
            if !metadata.is_dir() {
                return Err(source_shape_error(path, "not a readable directory"));
            }

            std::fs::read_dir(path)
                .map(|_| ())
                .map_err(|error| source_read_error(path, error))
        }
    }
}

pub(crate) fn list_directory_entries(path: &Path) -> Result<Vec<String>, SourceReadError> {
    logging::system(Level::Trace, "read_dir", format!("path={}", path.display()));

    let read_dir = std::fs::read_dir(path).map_err(|error| source_read_error(path, error))?;

    let mut entries = Vec::new();
    for entry in read_dir {
        let entry = entry.map_err(|error| source_read_error(path, error))?;

        let file_type = entry
            .file_type()
            .map_err(|error| source_read_error(path, error))?;

        if file_type.is_file() || file_type.is_symlink() {
            entries.push(entry.file_name().to_string_lossy().into_owned());
        }
    }

    entries.sort();
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_source_path_readable, list_directory_entries, resolve_home_directory,
        source_read_error, source_shape_error,
    };
    use crate::ProgramError;
    use crate::boundary::{SourcePathRequirement, SourceReadErrorKind};
    use std::collections::BTreeMap;

    use std::fs;
    use std::io::ErrorKind;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn resolve_home_directory_from_home_environment_variable() {
        let env_values = BTreeMap::from([("HOME".to_string(), "/tmp/home".to_string())]);
        let get_env_var = |name: &str| env_values.get(name).cloned();

        let result = resolve_home_directory(&get_env_var)
            .expect("expected resolver to return HOME when it is present");

        assert_eq!(result, PathBuf::from("/tmp/home"));
    }

    #[test]
    fn cannot_resolve_home_directory_when_the_home_environment_variable_is_missing() {
        let get_env_var = |_: &str| None::<String>;

        let error = resolve_home_directory(&get_env_var)
            .expect_err("expected resolver to fail when HOME is missing");

        assert_eq!(error, ProgramError::HomeDirectoryUnavailable);
    }

    #[test]
    fn list_directory_entries_include_files_only_in_sorted_order() {
        let directory = TempDir::new().expect("temporary directory should be created");
        fs::write(directory.path().join("z-last"), "z\n").expect("file should be written");
        fs::write(directory.path().join("a-first"), "a\n").expect("file should be written");
        fs::create_dir(directory.path().join("folder")).expect("directory should be written");

        let entries = list_directory_entries(directory.path())
            .expect("directory listing should succeed for readable directories");

        assert_eq!(entries, vec!["a-first".to_string(), "z-last".to_string()]);
    }

    #[test]
    fn report_read_source_directory_error_when_directory_cannot_be_read() {
        let missing_directory = PathBuf::from("/tmp/definitely-missing-gitenv-fs-adapter");

        let error = list_directory_entries(&missing_directory)
            .expect_err("listing should fail for a missing source directory");

        assert_eq!(error.path, missing_directory);
        assert_eq!(error.kind, SourceReadErrorKind::Missing);
    }

    #[test]
    fn report_missing_source_files_when_a_file_action_source_is_absent() {
        let missing_file = PathBuf::from("/tmp/definitely-missing-gitenv-source-file");

        let error = ensure_source_path_readable(&missing_file, SourcePathRequirement::File)
            .expect_err("missing files should be reported as unavailable");

        assert_eq!(error.path, missing_file);
        assert_eq!(error.kind, SourceReadErrorKind::Missing);
    }

    #[test]
    fn report_source_read_error_kinds_for_io_failures() {
        let path = PathBuf::from("/tmp/source-read-error");

        let permission_denied = source_read_error(
            &path,
            std::io::Error::new(ErrorKind::PermissionDenied, "permission denied"),
        );
        let unexpected_io = source_read_error(&path, std::io::Error::other("unexpected io"));

        assert_eq!(
            permission_denied.kind,
            SourceReadErrorKind::PermissionDenied
        );
        assert_eq!(unexpected_io.kind, SourceReadErrorKind::UnexpectedIo);
    }

    #[test]
    fn report_source_shape_errors_for_wrong_path_requirements() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let file_path = temp.path().join("source-file");
        let directory_path = temp.path().join("source-directory");
        fs::write(&file_path, "content\n").expect("file should be written");
        fs::create_dir_all(&directory_path).expect("directory should be created");

        let file_error = ensure_source_path_readable(&file_path, SourcePathRequirement::Directory)
            .expect_err("files should be rejected when a directory is required");
        let directory_error =
            ensure_source_path_readable(&directory_path, SourcePathRequirement::File)
                .expect_err("directories should be rejected when a file is required");

        assert_eq!(file_error.kind, SourceReadErrorKind::UnexpectedIo);
        assert_eq!(file_error.message, "not a readable directory");
        assert_eq!(directory_error.kind, SourceReadErrorKind::UnexpectedIo);
        assert_eq!(directory_error.message, "not a readable file");
        assert_eq!(
            source_shape_error(&file_path, "shape error").kind,
            SourceReadErrorKind::UnexpectedIo
        );
    }
}
