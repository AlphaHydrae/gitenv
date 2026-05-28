//! Shared boundary traits for external and system capabilities used across
//! planning and execution stages.
//!
//! Each trait abstracts one narrow system concern so that production code and
//! tests can swap implementations without touching domain logic.
//!
//! The composition root in `lib.rs` creates a single [`RealBoundary`] that
//! implements every trait by delegating to the real filesystem and environment.
//! Test code defines lightweight local doubles — simple structs or
//! closure wrappers — that are fast and deterministic without touching the
//! filesystem or process environment.
//!
//! Re-usable test doubles for these traits live in [`test_doubles`], which is
//! compiled only when running tests.

use crate::ProgramError;
use crate::actions;
use crate::config::{LoadedConfig, load_config};
use crate::fs_adapter;
use crate::logging;
use std::path::Path;
use std::path::PathBuf;

/// Declares which source-path shape and readability contract a planning stage
/// requires from [`SourceAvailabilityReader::ensure_source_path_readable`].
///
/// Operation planning uses this to enforce different source checks for copy,
/// symlink, and selector-root expansion paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourcePathRequirement {
    /// Source must be readable as a file-like path.
    ///
    /// This is used by copy actions, where directory inputs are invalid.
    File,
    /// Source must be readable as a directory.
    ///
    /// This is used when validating selector source roots before expansion.
    Directory,
    /// Source must be readable for symlink actions.
    ///
    /// Unlike [`File`], this allows either a readable file or a readable
    /// directory source because both are valid symlink targets.
    Symlink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceReadErrorKind {
    Missing,
    PermissionDenied,
    UnexpectedIo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceReadError {
    pub(crate) path: PathBuf,
    pub(crate) kind: SourceReadErrorKind,
    pub(crate) message: String,
}

/// Shared boundary for environment variable reads used across planning stages.
pub(crate) trait EnvironmentReader {
    fn get_env_var(&self, name: &str) -> Option<String>;
}

/// Shared boundary for probing whether a path points to an existing directory.
pub(crate) trait DirectoryProbe {
    fn is_directory(&self, path: &str) -> bool;
}

/// Shared boundary for reading and parsing configuration files.
pub(crate) trait ConfigReader {
    fn read_config_file(&self, path: &Path) -> Result<LoadedConfig, ProgramError>;
}

/// Shared boundary for checking whether a source path exists and is readable.
pub(crate) trait SourceAvailabilityReader {
    fn ensure_source_path_readable(
        &self,
        path: &Path,
        requirement: SourcePathRequirement,
    ) -> Result<(), SourceReadError>;
}

/// Shared boundary for probing whether an operation target currently exists.
pub(crate) trait TargetProbe {
    fn target_exists(&self, path: &Path) -> Result<bool, ProgramError>;
}

/// Shared boundary for creating symlinks during apply execution.
pub(crate) trait SymlinkCreator {
    fn create_symlink(&self, source: &Path, target: &Path) -> Result<(), ProgramError>;
}

/// Shared boundary for creating parent directories during apply execution.
pub(crate) trait DirectoryCreator {
    fn ensure_parent_directory_exists(&self, target: &Path) -> Result<(), ProgramError>;
}

/// Shared boundary for removing paths during apply execution.
pub(crate) trait PathRemover {
    fn remove_path(&self, path: &Path) -> Result<(), ProgramError>;
}

/// Production adapter implementation used by the composition root.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RealBoundary;

impl EnvironmentReader for RealBoundary {
    fn get_env_var(&self, name: &str) -> Option<String> {
        std::env::var_os(name).map(|v| v.to_string_lossy().into_owned())
    }
}

impl DirectoryProbe for RealBoundary {
    fn is_directory(&self, path: &str) -> bool {
        logging::system(log::Level::Trace, "metadata", format!("path={path}"));
        match std::fs::metadata(path) {
            Ok(metadata) => metadata.is_dir(),
            Err(_) => false,
        }
    }
}

impl ConfigReader for RealBoundary {
    fn read_config_file(&self, path: &Path) -> Result<LoadedConfig, ProgramError> {
        load_config(path)
    }
}

impl SourceAvailabilityReader for RealBoundary {
    fn ensure_source_path_readable(
        &self,
        path: &Path,
        requirement: SourcePathRequirement,
    ) -> Result<(), SourceReadError> {
        fs_adapter::ensure_source_path_readable(path, requirement)
    }
}

impl TargetProbe for RealBoundary {
    fn target_exists(&self, path: &Path) -> Result<bool, ProgramError> {
        actions::target_exists(path)
    }
}

impl SymlinkCreator for RealBoundary {
    fn create_symlink(&self, source: &Path, target: &Path) -> Result<(), ProgramError> {
        actions::create_symlink_on_filesystem(source, target)
    }
}

impl DirectoryCreator for RealBoundary {
    fn ensure_parent_directory_exists(&self, target: &Path) -> Result<(), ProgramError> {
        let Some(parent) = target.parent() else {
            return Ok(());
        };

        logging::system(
            log::Level::Trace,
            "create_dir_all",
            format!("path={}", parent.display()),
        );

        std::fs::create_dir_all(parent).map_err(|error| {
            ProgramError::TargetDirectoryCreationFailed {
                path: parent.to_path_buf(),
                message: error.to_string(),
            }
        })
    }
}

impl PathRemover for RealBoundary {
    fn remove_path(&self, path: &Path) -> Result<(), ProgramError> {
        logging::system(
            log::Level::Trace,
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
                log::Level::Trace,
                "remove_dir",
                format!("path={}", path.display()),
            );

            std::fs::remove_dir(path).map_err(|error| ProgramError::TargetRemovalFailed {
                path: path.to_path_buf(),
                message: error.to_string(),
            })
        } else {
            logging::system(
                log::Level::Trace,
                "remove_file",
                format!("path={}", path.display()),
            );

            std::fs::remove_file(path).map_err(|error| ProgramError::TargetRemovalFailed {
                path: path.to_path_buf(),
                message: error.to_string(),
            })
        }
    }
}

#[cfg(test)]
pub(crate) mod test_doubles {
    use super::{
        DirectoryCreator, EnvironmentReader, PathRemover, SourceAvailabilityReader,
        SourcePathRequirement, SourceReadError, SymlinkCreator, TargetProbe,
    };
    use crate::ProgramError;
    use std::collections::BTreeMap;
    use std::path::Path;

    /// Test double: returns environment variable values from a pre-populated
    /// map. Useful when tests need reproducible env reads without touching the
    /// real process environment.
    ///
    /// Prefer this over an ad-hoc local struct when a test only needs
    /// `EnvironmentReader` and the logic under test dispatches on specific
    /// variable names.
    pub(crate) struct MapEnvReader(pub(crate) BTreeMap<String, String>);

    impl MapEnvReader {
        /// Creates a reader pre-populated with the given key-value pairs.
        pub(crate) fn from_pairs(
            pairs: impl IntoIterator<Item = (&'static str, &'static str)>,
        ) -> Self {
            Self(
                pairs
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            )
        }

        /// Creates an empty reader that returns `None` for every variable.
        pub(crate) fn empty() -> Self {
            Self(BTreeMap::new())
        }
    }

    impl EnvironmentReader for MapEnvReader {
        fn get_env_var(&self, name: &str) -> Option<String> {
            self.0.get(name).cloned()
        }
    }

    /// Test double: wraps a closure for source readability checks.
    pub(crate) struct FnSourceAvailabilityReader<
        F: Fn(&Path, SourcePathRequirement) -> Result<(), SourceReadError>,
    >(pub(crate) F);

    impl<F: Fn(&Path, SourcePathRequirement) -> Result<(), SourceReadError>>
        SourceAvailabilityReader for FnSourceAvailabilityReader<F>
    {
        fn ensure_source_path_readable(
            &self,
            path: &Path,
            requirement: SourcePathRequirement,
        ) -> Result<(), SourceReadError> {
            self.0(path, requirement)
        }
    }

    /// Test double: wraps a closure for operation-target existence probes.
    pub(crate) struct FnTargetProbe<F: Fn(&Path) -> Result<bool, ProgramError>>(pub(crate) F);

    impl<F: Fn(&Path) -> Result<bool, ProgramError>> TargetProbe for FnTargetProbe<F> {
        fn target_exists(&self, path: &Path) -> Result<bool, ProgramError> {
            self.0(path)
        }
    }

    /// Test double: wraps a closure for symlink creation calls.
    pub(crate) struct FnSymlinkCreator<F: Fn(&Path, &Path) -> Result<(), ProgramError>>(
        pub(crate) F,
    );

    impl<F: Fn(&Path, &Path) -> Result<(), ProgramError>> SymlinkCreator for FnSymlinkCreator<F> {
        fn create_symlink(&self, source: &Path, target: &Path) -> Result<(), ProgramError> {
            self.0(source, target)
        }
    }

    /// Test double: wraps a closure for parent directory creation.
    pub(crate) struct FnDirectoryCreator<F: Fn(&Path) -> Result<(), ProgramError>>(pub(crate) F);

    impl<F: Fn(&Path) -> Result<(), ProgramError>> DirectoryCreator for FnDirectoryCreator<F> {
        fn ensure_parent_directory_exists(&self, target: &Path) -> Result<(), ProgramError> {
            self.0(target)
        }
    }

    /// Test double: wraps a closure for path removal.
    pub(crate) struct FnPathRemover<F: Fn(&Path) -> Result<(), ProgramError>>(pub(crate) F);

    impl<F: Fn(&Path) -> Result<(), ProgramError>> PathRemover for FnPathRemover<F> {
        fn remove_path(&self, path: &Path) -> Result<(), ProgramError> {
            self.0(path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ConfigReader, DirectoryCreator, DirectoryProbe, PathRemover, RealBoundary};
    use crate::{Config, LoadedConfig, ProgramError};
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    #[test]
    fn report_directory_probe_results_for_existing_and_missing_paths() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let boundary = RealBoundary;

        assert!(boundary.is_directory(&temp.path().to_string_lossy()));
        assert!(!boundary.is_directory(&temp.path().join("missing").to_string_lossy()));
    }

    #[test]
    fn read_config_files_through_the_boundary_adapter() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let config_path = temp.path().join("config.yml");
        let boundary = RealBoundary;
        fs::write(
            &config_path,
            concat!(
                "version: 1\n",
                "repository: \"/tmp/repo\"\n",
                "sources:\n",
                "  - from: \".\"\n",
                "    configs:\n",
                "      - file: .gitconfig\n"
            ),
        )
        .expect("config file should be written");

        let loaded_config = boundary
            .read_config_file(&config_path)
            .expect("boundary should read and parse the config file");

        assert_eq!(
            loaded_config,
            LoadedConfig {
                path: config_path,
                config: Config {
                    version: 1,
                    repository: "/tmp/repo".to_string(),
                    defaults: Default::default(),
                    includes: vec![],
                    sources: vec![crate::Source {
                        from: crate::SourceRoot::Path(".".to_string()),
                        to: None,
                        guard: None,
                        configs: vec![crate::ConfigItem::File(crate::FileConfig {
                            file: ".gitconfig".to_string(),
                            as_name: None,
                            mode: None,
                            to: None,
                            mkdir: None,
                            overwrite: None,
                            backup_on_overwrite: None,
                        })],
                    }],
                },
            }
        );
    }

    #[test]
    fn directory_creator_succeeds_for_parent_directory_creation() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let target = temp.path().join("nested").join("target.txt");
        let boundary = RealBoundary;

        let result = boundary.ensure_parent_directory_exists(&target);

        assert!(
            result.is_ok(),
            "directory creation should succeed for valid paths"
        );
        assert!(target.parent().unwrap().exists());
    }

    #[test]
    fn directory_creator_reports_failures_when_parent_is_a_file() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let parent_file = temp.path().join("parent-as-file");
        fs::write(&parent_file, "content\n").expect("file should be written");
        let nested_target = parent_file.join("target.txt");
        let boundary = RealBoundary;

        let error = boundary
            .ensure_parent_directory_exists(&nested_target)
            .expect_err("mkdir should fail when parent is a file");

        assert!(matches!(
            &error,
            ProgramError::TargetDirectoryCreationFailed { path, .. } if path == &parent_file
        ));
    }

    #[test]
    fn path_remover_succeeds_for_file_removal() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let target_file = temp.path().join("target.txt");
        fs::write(&target_file, "content\n").expect("file should be written");
        let boundary = RealBoundary;

        let result = boundary.remove_path(&target_file);

        assert!(result.is_ok(), "file removal should succeed");
        assert!(!target_file.exists());
    }

    #[test]
    fn path_remover_succeeds_for_directory_removal() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let target_dir = temp.path().join("target-dir");
        fs::create_dir(&target_dir).expect("directory should be created");
        let boundary = RealBoundary;

        let result = boundary.remove_path(&target_dir);

        assert!(result.is_ok(), "directory removal should succeed");
        assert!(!target_dir.exists());
    }

    #[test]
    fn path_remover_reports_failures_for_missing_paths() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let missing_path = temp.path().join("missing");
        let boundary = RealBoundary;

        let error = boundary
            .remove_path(&missing_path)
            .expect_err("remove should fail for missing paths");

        assert!(matches!(
            &error,
            ProgramError::TargetRemovalFailed { path, .. } if path == &missing_path
        ));
    }

    #[test]
    fn directory_creator_handles_root_path_with_no_parent() {
        let boundary = RealBoundary;
        // On Unix, "/" has no parent. On Windows, a root like "C:\" has a parent "C:".
        // So to test the no-parent case, we pass a path that Path::parent() returns None for.
        // In practice, this only happens with paths like "/" on Unix.
        #[cfg(unix)]
        {
            let root = std::path::Path::new("/");
            let result = boundary.ensure_parent_directory_exists(root);
            assert!(result.is_ok(), "should handle root path with no parent");
        }
    }

    #[test]
    #[cfg(unix)]
    fn path_remover_reports_permission_denied_on_directory_removal() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let target_dir = temp.path().join("target-dir");
        fs::create_dir(&target_dir).expect("directory should be created");

        // Make parent directory read-only to prevent directory removal
        let parent = temp.path();
        let perms = fs::Permissions::from_mode(0o555);
        fs::set_permissions(parent, perms).expect("permissions should be set");

        let boundary = RealBoundary;
        let error = boundary
            .remove_path(&target_dir)
            .expect_err("remove should fail without write permissions");

        // Restore permissions for cleanup
        let perms = fs::Permissions::from_mode(0o755);
        fs::set_permissions(parent, perms).expect("permissions should be restored");

        assert!(matches!(
            &error,
            ProgramError::TargetRemovalFailed { path, .. } if path == &target_dir
        ));
    }

    #[test]
    #[cfg(unix)]
    fn path_remover_reports_permission_denied_on_file_removal() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let target_file = temp.path().join("target.txt");
        fs::write(&target_file, "content\n").expect("file should be written");

        // Make parent directory read-only to prevent file removal
        let parent = temp.path();
        let perms = fs::Permissions::from_mode(0o555);
        fs::set_permissions(parent, perms).expect("permissions should be set");

        let boundary = RealBoundary;
        let error = boundary
            .remove_path(&target_file)
            .expect_err("remove should fail without write permissions");

        // Restore permissions for cleanup
        let perms = fs::Permissions::from_mode(0o755);
        fs::set_permissions(parent, perms).expect("permissions should be restored");

        assert!(matches!(
            &error,
            ProgramError::TargetRemovalFailed { path, .. } if path == &target_file
        ));
    }
}
