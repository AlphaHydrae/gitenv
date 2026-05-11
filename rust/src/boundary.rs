use crate::ProgramError;
use crate::actions;
use crate::config::{LoadedConfig, load_config};
use crate::fs_adapter;
use crate::logging;
use std::path::Path;

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
    fn list_directory_entries(&self, path: &Path) -> Result<Vec<String>, ProgramError>;
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
    fn list_directory_entries(&self, path: &Path) -> Result<Vec<String>, ProgramError> {
        fs_adapter::list_directory_entries(path)
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
