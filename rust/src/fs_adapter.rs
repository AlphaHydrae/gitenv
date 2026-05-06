use crate::ProgramError;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;

pub(crate) fn resolve_home_directory(
    get_env_var_os: &dyn Fn(&str) -> Option<OsString>,
) -> Result<PathBuf, ProgramError> {
    get_env_var_os("HOME")
        .map(PathBuf::from)
        .ok_or(ProgramError::HomeDirectoryUnavailable)
}

fn read_source_directory_error(path: &Path, error: impl ToString) -> ProgramError {
    ProgramError::ReadSourceDirectory {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

pub(crate) fn list_directory_entries(path: &Path) -> Result<Vec<String>, ProgramError> {
    let read_dir =
        std::fs::read_dir(path).map_err(|error| read_source_directory_error(path, error))?;

    let mut entries = Vec::new();
    for entry in read_dir {
        let entry = entry.map_err(|error| read_source_directory_error(path, error))?;

        let file_type = entry
            .file_type()
            .map_err(|error| read_source_directory_error(path, error))?;

        if file_type.is_file() || file_type.is_symlink() {
            entries.push(entry.file_name().to_string_lossy().into_owned());
        }
    }

    entries.sort();
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::resolve_home_directory;
    use crate::ProgramError;
    use std::collections::BTreeMap;
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn resolve_home_directory_from_home_environment_variable() {
        let env_values = BTreeMap::from([("HOME".to_string(), OsString::from("/tmp/home"))]);
        let get_env_var_os = |name: &str| env_values.get(name).cloned();

        let result = resolve_home_directory(&get_env_var_os)
            .expect("expected resolver to return HOME when it is present");

        assert_eq!(result, PathBuf::from("/tmp/home"));
    }

    #[test]
    fn return_home_directory_unavailable_when_home_environment_variable_is_missing() {
        let get_env_var_os = |_: &str| None::<OsString>;

        let error = resolve_home_directory(&get_env_var_os)
            .expect_err("expected resolver to fail when HOME is missing");

        assert_eq!(error, ProgramError::HomeDirectoryUnavailable);
    }
}
