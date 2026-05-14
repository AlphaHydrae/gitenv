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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourcePathRequirement {
    File,
    Directory,
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

/// Shared boundary for enumerating file entries in a directory.
pub(crate) trait DirectoryEntriesReader {
    fn list_directory_entries(&self, path: &Path) -> Result<Vec<String>, SourceReadError>;
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

impl DirectoryEntriesReader for RealBoundary {
    fn list_directory_entries(&self, path: &Path) -> Result<Vec<String>, SourceReadError> {
        fs_adapter::list_directory_entries(path)
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

#[cfg(test)]
pub(crate) mod test_doubles {
    use super::{
        DirectoryEntriesReader, EnvironmentReader, SourceAvailabilityReader, SourcePathRequirement,
        SourceReadError, SymlinkCreator, TargetProbe,
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

    /// Test double: wraps a closure for directory listing operations.
    pub(crate) struct FnDirectoryReader<F: Fn(&Path) -> Result<Vec<String>, SourceReadError>>(
        pub(crate) F,
    );

    impl<F: Fn(&Path) -> Result<Vec<String>, SourceReadError>> DirectoryEntriesReader
        for FnDirectoryReader<F>
    {
        fn list_directory_entries(&self, path: &Path) -> Result<Vec<String>, SourceReadError> {
            self.0(path)
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
}

#[cfg(test)]
mod tests {
    use super::{ConfigReader, DirectoryProbe, RealBoundary};
    use crate::{Config, LoadedConfig};
    use std::fs;
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
}
