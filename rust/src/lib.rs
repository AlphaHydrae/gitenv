mod actions;
mod cli;
mod color;
mod config;
mod fs_adapter;
mod intent;
mod logging;
mod operation;
mod status;

pub use cli::{Cli, Command, LogLevel};
pub use color::{ColorMode, RuntimeConfig};
pub use logging::init as init_logging;

pub use actions::{
    ApplyOperationOutcome, ApplyOperationReport, apply_operation_plan,
    apply_operation_plan_with_injectables,
};
pub use config::{
    ActionMode, Config, ConfigItem, Defaults, FileConfig, Guard, Include, LoadedConfig,
    SelectConfig, Source, SourceRoot, load_config, parse_config,
};
pub use intent::{
    ConflictPolicy, IntentAction, IntentFileAction, IntentPlan, IntentSelectAction, IntentSource,
    ResolvedOptions, derive_intent_plan, derive_intent_plan_with_env,
    derive_intent_plan_with_env_and_fs, derive_intent_plan_with_injectables,
};
pub use operation::{
    FileOperation, OperationAction, OperationPlan, derive_operation_plan,
    derive_operation_plan_with_injectables,
};
pub use status::{
    CopyInspection, CopyInspectionState, OperationInspectionOutcome, OperationInspectionReport,
    SymlinkInspection, SymlinkInspectionState, inspect_operation_plan_status,
    inspect_symlink_operation_status,
};

use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramOutput {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramError {
    InvalidConfiguration {
        message: String,
    },
    ReadConfiguration {
        path: PathBuf,
        message: String,
    },
    MissingEnvironment {
        vars: Vec<String>,
    },
    /// One or more required include files could not be found. The paths are
    /// sorted for deterministic output.
    IncludeNotFound {
        paths: Vec<PathBuf>,
    },
    /// A config file was encountered more than once in the same include chain,
    /// forming a cycle. Contains the full cycle path in traversal order,
    /// ending with the repeated path that closes the cycle.
    IncludeCycle {
        cycle: Vec<PathBuf>,
    },
    /// A source directory could not be read while expanding selectors.
    ReadSourceDirectory {
        path: PathBuf,
        message: String,
    },
    /// A filesystem path could not be inspected while deriving status.
    InspectPathMetadata {
        path: PathBuf,
        message: String,
    },
    /// A symlink target could not be read while deriving status.
    ReadSymlinkTarget {
        path: PathBuf,
        message: String,
    },
    /// A symlink could not be created while applying operations.
    CreateSymlink {
        source: PathBuf,
        target: PathBuf,
        message: String,
    },
    /// A file copy operation could not be completed while applying operations.
    CopyFile {
        source: PathBuf,
        target: PathBuf,
        message: String,
    },
    /// The target's parent directory could not be created before apply.
    CreateTargetDirectory {
        path: PathBuf,
        message: String,
    },
    /// An existing target could not be removed before overwrite.
    RemoveTarget {
        path: PathBuf,
        message: String,
    },
    /// An existing target could not be moved to its backup path.
    BackupTarget {
        path: PathBuf,
        backup_path: PathBuf,
        message: String,
    },
    /// Backup-on-overwrite cannot proceed because the backup path exists.
    BackupAlreadyExists {
        path: PathBuf,
    },
    /// The current home directory is required to resolve home-relative paths.
    HomeDirectoryUnavailable,
}

const DEFAULT_CONFIG_HOME_SUFFIX: &str = ".config";
const DEFAULT_CONFIG_DIRECTORY_NAME: &str = "gitenv";
const DEFAULT_CONFIG_FILE_NAME: &str = "config.yml";

#[derive(Clone, Copy)]
struct SystemCalls<'a> {
    get_env_var_os: &'a dyn Fn(&str) -> Option<OsString>,
}

fn std_env_var_os(name: &str) -> Option<OsString> {
    std::env::var_os(name)
}

fn real_system_calls() -> SystemCalls<'static> {
    SystemCalls {
        get_env_var_os: &std_env_var_os,
    }
}

/// Parse CLI arguments and dispatch to the appropriate library function.
///
/// This is the main entry point for the binary.
pub fn run_cli() -> Result<ProgramOutput, ProgramError> {
    use clap::Parser;
    use std::io::IsTerminal;

    let args = std::env::args_os();
    let cli = Cli::parse_from(args);

    let runtime_config = RuntimeConfig::new(
        cli.color,
        std::io::stdout().is_terminal(),
        std::io::stderr().is_terminal(),
    );

    logging::init(
        cli.log_level.clone().into(),
        runtime_config.use_color_for_stderr,
    );

    let system = real_system_calls();

    cli::dispatch_with(
        cli,
        || {
            run_info(
                || load_default_config_with_system(system),
                system,
                runtime_config,
            )
        },
        || {
            run_apply(
                || load_default_config_with_system(system),
                system,
                runtime_config,
            )
        },
    )
}

fn run_info(
    load_config: impl FnOnce() -> Result<LoadedConfig, ProgramError>,
    system: SystemCalls<'_>,
    runtime_config: RuntimeConfig,
) -> Result<ProgramOutput, ProgramError> {
    let loaded_config = load_config()?;
    let intent_plan = derive_intent_plan(&loaded_config)?;
    let home_directory = fs_adapter::resolve_home_directory(system.get_env_var_os)?;
    let operation_plan = operation::derive_operation_plan_with_injectables(
        &intent_plan,
        &home_directory,
        &fs_adapter::list_directory_entries,
    )?;

    Ok(ProgramOutput {
        message: cli::render_default_inspection_output(
            &operation_plan,
            &home_directory,
            runtime_config.use_color_for_stdout,
        )?,
    })
}

fn run_apply(
    load_config: impl FnOnce() -> Result<LoadedConfig, ProgramError>,
    system: SystemCalls<'_>,
    runtime_config: RuntimeConfig,
) -> Result<ProgramOutput, ProgramError> {
    let loaded_config = load_config()?;
    let intent_plan = derive_intent_plan(&loaded_config)?;
    let home_directory = fs_adapter::resolve_home_directory(system.get_env_var_os)?;
    let operation_plan = operation::derive_operation_plan_with_injectables(
        &intent_plan,
        &home_directory,
        &fs_adapter::list_directory_entries,
    )?;

    let apply_report = apply_operation_plan(&operation_plan)?;
    let output_message = cli::render_apply_output(
        &apply_report,
        &home_directory,
        runtime_config.use_color_for_stdout,
    );

    Ok(ProgramOutput {
        message: output_message,
    })
}

fn load_default_config_with_system(system: SystemCalls<'_>) -> Result<LoadedConfig, ProgramError> {
    let config_path = default_config_path_with_system(system)?;
    load_config(&config_path)
}

fn default_config_path_with_system(system: SystemCalls<'_>) -> Result<PathBuf, ProgramError> {
    let gitenv_config = (system.get_env_var_os)("GITENV_CONFIG").map(PathBuf::from);
    let home_directory = (system.get_env_var_os)("HOME").map(PathBuf::from);
    let xdg_config_home = (system.get_env_var_os)("XDG_CONFIG_HOME").map(PathBuf::from);

    default_config_path_from_inputs(gitenv_config, home_directory, xdg_config_home)
}

fn default_config_path_from_inputs(
    gitenv_config: Option<PathBuf>,
    home_directory: Option<PathBuf>,
    xdg_config_home: Option<PathBuf>,
) -> Result<PathBuf, ProgramError> {
    if let Some(custom_path) = gitenv_config {
        return Ok(custom_path);
    }

    let home_directory = home_directory.ok_or(ProgramError::HomeDirectoryUnavailable)?;

    Ok(default_config_path_from_env(
        &home_directory,
        xdg_config_home,
    ))
}

fn default_config_path_from_env(
    home_directory: &Path,
    xdg_config_home: Option<PathBuf>,
) -> PathBuf {
    let config_home = xdg_config_home
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home_directory.join(DEFAULT_CONFIG_HOME_SUFFIX));

    config_home
        .join(DEFAULT_CONFIG_DIRECTORY_NAME)
        .join(DEFAULT_CONFIG_FILE_NAME)
}

impl fmt::Display for ProgramError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProgramError::InvalidConfiguration { message } => {
                write!(f, "configuration is invalid ({message})")
            }
            ProgramError::ReadConfiguration { path, message } => {
                let setup_guidance = concat!(
                    "\n\nConfig file locations\n",
                    "  - $GITENV_CONFIG (if set)\n",
                    "  - ${XDG_CONFIG_HOME}/gitenv/config.yml (if $XDG_CONFIG_HOME is set)\n",
                    "  - ${HOME}/.config/gitenv/config.yml (default fallback)"
                );
                write!(
                    f,
                    "cannot read config at {} ({message}){}",
                    path.display(),
                    setup_guidance,
                )
            }
            ProgramError::MissingEnvironment { vars } => {
                write!(
                    f,
                    "required environment variables are missing ({})",
                    vars.join(", ")
                )
            }
            ProgramError::IncludeNotFound { paths } => write!(
                f,
                "required include files are missing\n  - {}",
                paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n  - ")
            ),
            ProgramError::IncludeCycle { cycle } => write!(
                f,
                "include cycle is detected\n  - {}",
                cycle
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n  - ")
            ),
            ProgramError::ReadSourceDirectory { path, message } => {
                write!(
                    f,
                    "cannot read source directory {} ({message})",
                    path.display()
                )
            }
            ProgramError::InspectPathMetadata { path, message } => {
                write!(
                    f,
                    "cannot inspect path metadata for {} ({message})",
                    path.display()
                )
            }
            ProgramError::ReadSymlinkTarget { path, message } => {
                write!(
                    f,
                    "cannot read symlink target for {} ({message})",
                    path.display()
                )
            }
            ProgramError::CreateSymlink {
                source,
                target,
                message,
            } => {
                write!(
                    f,
                    "cannot create symlink {} -> {} ({message})",
                    target.display(),
                    source.display(),
                )
            }
            ProgramError::CopyFile {
                source,
                target,
                message,
            } => {
                write!(
                    f,
                    "cannot copy file {} -> {} ({message})",
                    source.display(),
                    target.display(),
                )
            }
            ProgramError::CreateTargetDirectory { path, message } => {
                write!(
                    f,
                    "cannot create target directory {} ({message})",
                    path.display()
                )
            }
            ProgramError::RemoveTarget { path, message } => {
                write!(
                    f,
                    "cannot remove existing target {} ({message})",
                    path.display()
                )
            }
            ProgramError::BackupTarget {
                path,
                backup_path,
                message,
            } => {
                write!(
                    f,
                    "cannot move existing target {} to backup {} ({message})",
                    path.display(),
                    backup_path.display(),
                )
            }
            ProgramError::BackupAlreadyExists { path } => {
                write!(
                    f,
                    "cannot overwrite because backup already exists at {}",
                    path.display()
                )
            }
            ProgramError::HomeDirectoryUnavailable => {
                write!(f, "cannot resolve home directory from $HOME")
            }
        }
    }
}

impl std::error::Error for ProgramError {}

#[cfg(test)]
mod tests {
    use super::{
        ActionMode, ColorMode, Config, ConfigItem, Defaults, FileConfig, LoadedConfig,
        ProgramError, RuntimeConfig, Source, SourceRoot, SystemCalls, default_config_path_from_env,
        default_config_path_from_inputs, load_config, load_default_config_with_system, run_apply,
        run_info,
    };
    use std::collections::BTreeMap;
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    fn make_config(repository: &Path, sources: Vec<Source>) -> Config {
        Config {
            version: 1,
            repository: repository.display().to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources,
        }
    }

    fn make_source(destination: &Path, configs: Vec<ConfigItem>) -> Source {
        Source {
            from: SourceRoot::Path(".".to_string()),
            to: Some(destination.display().to_string()),
            guard: None,
            configs,
        }
    }

    fn make_file_config(file: &str, mode: ActionMode) -> ConfigItem {
        ConfigItem::File(FileConfig {
            file: file.to_string(),
            as_name: None,
            mode: Some(mode),
            to: None,
            mkdir: None,
            overwrite: None,
            backup_on_overwrite: None,
        })
    }

    fn loaded_config_for_test(config: Config) -> LoadedConfig {
        LoadedConfig {
            path: PathBuf::from("/tmp/gitenv-test-config.yml"),
            config,
        }
    }

    #[test]
    fn resolve_default_config_path_from_xdg_config_home() {
        let path = default_config_path_from_env(
            Path::new("/home/alex"),
            Some(PathBuf::from("/tmp/runtime-config")),
        );

        assert_eq!(path, PathBuf::from("/tmp/runtime-config/gitenv/config.yml"));
    }

    #[test]
    fn resolve_default_config_path_from_home_config_when_xdg_is_unset_or_relative() {
        let fallback_path = default_config_path_from_env(Path::new("/home/alex"), None);
        let relative_xdg_path = default_config_path_from_env(
            Path::new("/home/alex"),
            Some(PathBuf::from("relative/config")),
        );

        assert_eq!(
            fallback_path,
            PathBuf::from("/home/alex/.config/gitenv/config.yml")
        );
        assert_eq!(
            relative_xdg_path,
            PathBuf::from("/home/alex/.config/gitenv/config.yml")
        );
    }

    #[test]
    fn resolve_config_path_from_gitenv_config_override() {
        let path = default_config_path_from_inputs(
            Some(PathBuf::from("/tmp/custom/config.yml")),
            Some(PathBuf::from("/home/alex")),
            Some(PathBuf::from("/tmp/runtime-config")),
        )
        .expect("GITENV_CONFIG override should be used");

        assert_eq!(path, PathBuf::from("/tmp/custom/config.yml"));
    }

    #[test]
    fn cannot_resolve_default_config_path_without_home_or_override() {
        let error = default_config_path_from_inputs(None, None, None)
            .expect_err("HOME is required when GITENV_CONFIG is absent");

        assert_eq!(error, ProgramError::HomeDirectoryUnavailable);
    }

    #[test]
    fn show_default_inspection_output_for_a_missing_symlink() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        fs::write(repository.path().join(".gitconfig"), "[user]\n")
            .expect("source file should be written");

        let config = format!(
            concat!(
                "version: 1\n",
                "repository: \"{}\"\n",
                "sources:\n",
                "  - from: \".\"\n",
                "    to: \"{}\"\n",
                "    configs:\n",
                "      - file: .gitconfig\n"
            ),
            repository.path().display(),
            home.path().display()
        );
        let config_path = home.path().join("config.yml");
        fs::write(&config_path, config).expect("config file should be written");

        let env_values =
            BTreeMap::from([("HOME".to_string(), OsString::from(home.path().as_os_str()))]);
        let get_env_var_os = move |name: &str| env_values.get(name).cloned();
        let output = run_info(
            || load_config(&config_path),
            SystemCalls {
                get_env_var_os: &get_env_var_os,
            },
            RuntimeConfig::new(ColorMode::Auto, false, false),
        )
        .expect("explicit config path should produce inspection output");

        let expected = format!(
            "~/.gitconfig -> {}   not yet set up",
            repository.path().join(".").join(".gitconfig").display()
        );
        assert_eq!(output.message, expected);
    }

    #[test]
    fn show_default_inspection_output_when_config_path_is_provided_for_relative_include() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        fs::write(repository.path().join("root.conf"), "root\n")
            .expect("root source file should be written");
        fs::write(repository.path().join("shared.conf"), "shared\n")
            .expect("shared source file should be written");

        let config_directory = home.path().join("configs");
        let root_config_path = config_directory.join("root.yml");
        let shared_config_path = config_directory.join("includes").join("shared.yml");
        let root_config = format!(
            concat!(
                "version: 1\n",
                "repository: \"{}\"\n",
                "includes:\n",
                "  - includes/shared.yml\n",
                "sources:\n",
                "  - from: \".\"\n",
                "    configs:\n",
                "      - file: root.conf\n",
                "        as: .root.conf\n"
            ),
            repository.path().display()
        );
        let shared_config = format!(
            concat!(
                "version: 1\n",
                "repository: \"{}\"\n",
                "sources:\n",
                "  - from: \".\"\n",
                "    configs:\n",
                "      - file: shared.conf\n",
                "        as: .shared.conf\n"
            ),
            repository.path().display()
        );
        fs::create_dir_all(
            shared_config_path
                .parent()
                .expect("shared config should have a parent directory"),
        )
        .expect("shared config directory should be created");
        fs::write(&root_config_path, root_config).expect("root config should be written");
        fs::write(&shared_config_path, shared_config).expect("shared config should be written");

        let env_values =
            BTreeMap::from([("HOME".to_string(), OsString::from(home.path().as_os_str()))]);
        let get_env_var_os = move |name: &str| env_values.get(name).cloned();
        let output = run_info(
            || load_config(&root_config_path),
            SystemCalls {
                get_env_var_os: &get_env_var_os,
            },
            RuntimeConfig::new(ColorMode::Auto, false, false),
        )
        .expect("run_info should resolve relative include paths from the provided config path");

        let expected = format!(
            concat!(
                "~/.root.conf -> {}   not yet set up\n",
                "~/.shared.conf -> {}   not yet set up"
            ),
            repository.path().join(".").join("root.conf").display(),
            repository.path().join(".").join("shared.conf").display(),
        );
        assert_eq!(output.message, expected);
    }

    #[test]
    fn propagate_copy_source_read_failures_from_default_inspection() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        let config = make_config(
            repository.path(),
            vec![make_source(
                home.path(),
                vec![make_file_config("missing-source.txt", ActionMode::Copy)],
            )],
        );
        fs::write(home.path().join("missing-source.txt"), "target exists\n")
            .expect("copy target should exist so inspection attempts source hashing");

        let env_values =
            BTreeMap::from([("HOME".to_string(), OsString::from(home.path().as_os_str()))]);
        let get_env_var_os = move |name: &str| env_values.get(name).cloned();

        let error = run_info(
            || Ok(loaded_config_for_test(config)),
            SystemCalls {
                get_env_var_os: &get_env_var_os,
            },
            RuntimeConfig::new(ColorMode::Auto, false, false),
        )
        .expect_err("missing copy source should propagate an inspection error");

        assert!(matches!(
            error,
            ProgramError::InspectPathMetadata { path, .. }
                if path == repository.path().join(".").join("missing-source.txt")
        ));
    }

    #[test]
    fn cannot_run_info_when_source_environment_is_missing() {
        let config = Config {
            version: 1,
            repository: "/repo".to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Environment {
                    env: "MISSING_SOURCE_ROOT".to_string(),
                    optional: false,
                },
                to: Some("/tmp".to_string()),
                guard: None,
                configs: vec![make_file_config(".gitconfig", ActionMode::Symlink)],
            }],
        };

        let get_env_var_os = |_: &str| None::<OsString>;
        let error = run_info(
            || Ok(loaded_config_for_test(config)),
            SystemCalls {
                get_env_var_os: &get_env_var_os,
            },
            RuntimeConfig::new(ColorMode::Auto, false, false),
        )
        .expect_err("missing source environment variable should fail info planning");

        assert_eq!(
            error,
            ProgramError::MissingEnvironment {
                vars: vec!["MISSING_SOURCE_ROOT".to_string()],
            }
        );
    }

    #[test]
    fn cannot_run_apply_when_source_environment_is_missing() {
        let config = Config {
            version: 1,
            repository: "/repo".to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Environment {
                    env: "MISSING_SOURCE_ROOT".to_string(),
                    optional: false,
                },
                to: Some("/tmp".to_string()),
                guard: None,
                configs: vec![make_file_config(".gitconfig", ActionMode::Symlink)],
            }],
        };

        let get_env_var_os = |_: &str| None::<OsString>;
        let error = run_apply(
            || Ok(loaded_config_for_test(config)),
            SystemCalls {
                get_env_var_os: &get_env_var_os,
            },
            RuntimeConfig::new(ColorMode::Auto, false, false),
        )
        .expect_err("missing source environment variable should fail apply planning");

        assert_eq!(
            error,
            ProgramError::MissingEnvironment {
                vars: vec!["MISSING_SOURCE_ROOT".to_string()],
            }
        );
    }

    #[test]
    fn show_setup_guidance_when_file_is_missing() {
        let missing_path = PathBuf::from("/tmp/custom-gitenv.yml");

        let get_env_var_os = |_: &str| None::<OsString>;
        let error = run_info(
            || load_config(&missing_path),
            SystemCalls {
                get_env_var_os: &get_env_var_os,
            },
            RuntimeConfig::new(ColorMode::Auto, false, false),
        )
        .expect_err("missing explicit config file should return a read error");

        let rendered = error.to_string();
        assert!(rendered.contains("cannot read config at /tmp/custom-gitenv.yml ("));
        assert!(rendered.contains("Config file locations"));
        assert!(rendered.contains("$GITENV_CONFIG (if set)"));
    }

    #[test]
    fn show_apply_output_for_a_missing_symlink() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        fs::write(repository.path().join(".gitconfig"), "[user]\n")
            .expect("source file should be written");

        let config = format!(
            concat!(
                "version: 1\n",
                "repository: \"{}\"\n",
                "sources:\n",
                "  - from: \".\"\n",
                "    to: \"{}\"\n",
                "    configs:\n",
                "      - file: .gitconfig\n"
            ),
            repository.path().display(),
            home.path().display()
        );
        let config_path = home.path().join("config.yml");
        fs::write(&config_path, config).expect("config file should be written");

        let env_values =
            BTreeMap::from([("HOME".to_string(), OsString::from(home.path().as_os_str()))]);
        let get_env_var_os = move |name: &str| env_values.get(name).cloned();
        let output = run_apply(
            || load_config(&config_path),
            SystemCalls {
                get_env_var_os: &get_env_var_os,
            },
            RuntimeConfig::new(ColorMode::Auto, false, false),
        )
        .expect("explicit config path should produce apply output");

        let expected = format!(
            "created symlink ~/.gitconfig -> {}",
            repository.path().join(".").join(".gitconfig").display()
        );
        assert_eq!(output.message, expected);
    }

    #[test]
    fn show_apply_output_when_config_path_is_provided_for_relative_include() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        fs::write(repository.path().join("root.conf"), "root\n")
            .expect("root source file should be written");
        fs::write(repository.path().join("shared.conf"), "shared\n")
            .expect("shared source file should be written");

        let config_directory = home.path().join("configs");
        let root_config_path = config_directory.join("root.yml");
        let shared_config_path = config_directory.join("includes").join("shared.yml");
        let root_config = format!(
            concat!(
                "version: 1\n",
                "repository: \"{}\"\n",
                "includes:\n",
                "  - includes/shared.yml\n",
                "sources:\n",
                "  - from: \".\"\n",
                "    configs:\n",
                "      - file: root.conf\n",
                "        as: .root.conf\n"
            ),
            repository.path().display()
        );
        let shared_config = format!(
            concat!(
                "version: 1\n",
                "repository: \"{}\"\n",
                "sources:\n",
                "  - from: \".\"\n",
                "    configs:\n",
                "      - file: shared.conf\n",
                "        as: .shared.conf\n"
            ),
            repository.path().display()
        );
        fs::create_dir_all(
            shared_config_path
                .parent()
                .expect("shared config should have a parent directory"),
        )
        .expect("shared config directory should be created");
        fs::write(&root_config_path, root_config).expect("root config should be written");
        fs::write(&shared_config_path, shared_config).expect("shared config should be written");

        let env_values =
            BTreeMap::from([("HOME".to_string(), OsString::from(home.path().as_os_str()))]);
        let get_env_var_os = move |name: &str| env_values.get(name).cloned();
        let output = run_apply(
            || load_config(&root_config_path),
            SystemCalls {
                get_env_var_os: &get_env_var_os,
            },
            RuntimeConfig::new(ColorMode::Auto, false, false),
        )
        .expect("run_apply should resolve relative include paths from the provided config path");

        let expected = format!(
            concat!(
                "created symlink ~/.root.conf -> {}\n",
                "created symlink ~/.shared.conf -> {}"
            ),
            repository.path().join(".").join("root.conf").display(),
            repository.path().join(".").join("shared.conf").display(),
        );
        assert_eq!(output.message, expected);
    }

    #[test]
    fn propagate_copy_failures_from_apply_execution() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        let config = make_config(
            repository.path(),
            vec![make_source(
                home.path(),
                vec![make_file_config("missing-source.txt", ActionMode::Copy)],
            )],
        );

        let env_values =
            BTreeMap::from([("HOME".to_string(), OsString::from(home.path().as_os_str()))]);
        let get_env_var_os = move |name: &str| env_values.get(name).cloned();
        let error = run_apply(
            || Ok(loaded_config_for_test(config)),
            SystemCalls {
                get_env_var_os: &get_env_var_os,
            },
            RuntimeConfig::new(ColorMode::Auto, false, false),
        )
        .expect_err("missing copy source should fail apply execution");

        assert!(matches!(
            error,
            ProgramError::CopyFile { source, target, .. }
                if source == repository.path().join(".").join("missing-source.txt")
                    && target == home.path().join("missing-source.txt")
        ));
    }

    #[test]
    fn show_home_missing_errors_for_explicit_config_info_and_apply() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        fs::write(repository.path().join(".gitconfig"), "[user]\n")
            .expect("source file should be written");
        let info_config = make_config(
            repository.path(),
            vec![make_source(
                home.path(),
                vec![make_file_config(".gitconfig", ActionMode::Symlink)],
            )],
        );
        let apply_config = make_config(
            repository.path(),
            vec![make_source(
                home.path(),
                vec![make_file_config(".gitconfig", ActionMode::Symlink)],
            )],
        );

        let get_env_var_os = |_: &str| None::<OsString>;

        let info_error = run_info(
            || Ok(loaded_config_for_test(info_config)),
            SystemCalls {
                get_env_var_os: &get_env_var_os,
            },
            RuntimeConfig::new(ColorMode::Auto, false, false),
        )
        .expect_err("missing HOME should fail explicit-config info planning");
        assert_eq!(info_error, ProgramError::HomeDirectoryUnavailable);

        let apply_error = run_apply(
            || Ok(loaded_config_for_test(apply_config)),
            SystemCalls {
                get_env_var_os: &get_env_var_os,
            },
            RuntimeConfig::new(ColorMode::Auto, false, false),
        )
        .expect_err("missing HOME should fail explicit-config apply planning");
        assert_eq!(apply_error, ProgramError::HomeDirectoryUnavailable);
    }

    #[test]
    fn show_default_inspection_output() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        fs::write(repository.path().join(".profile"), "# profile\n")
            .expect("source file should be written");

        let config = format!(
            concat!(
                "version: 1\n",
                "repository: \"{}\"\n",
                "sources:\n",
                "  - from: \".\"\n",
                "    to: \"{}\"\n",
                "    configs:\n",
                "      - file: .profile\n"
            ),
            repository.path().display(),
            home.path().display()
        );
        let config_path = home.path().join("gitenv.yml");
        fs::write(&config_path, config).expect("config file should be written");

        let env_values =
            BTreeMap::from([("HOME".to_string(), OsString::from(home.path().as_os_str()))]);
        let get_env_var_os = move |name: &str| env_values.get(name).cloned();
        let output = run_info(
            || load_config(&config_path),
            SystemCalls {
                get_env_var_os: &get_env_var_os,
            },
            RuntimeConfig::new(ColorMode::Auto, false, false),
        )
        .expect("run_info_from_env should succeed with injected config path");

        let expected = format!(
            "~/.profile -> {}   not yet set up",
            repository.path().join(".").join(".profile").display()
        );
        assert_eq!(output.message, expected);
    }

    #[test]
    fn show_apply_output_from_wrapper_with_injected_config_loader() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        fs::write(repository.path().join(".profile"), "# profile\n")
            .expect("source file should be written");

        let config = format!(
            concat!(
                "version: 1\n",
                "repository: \"{}\"\n",
                "sources:\n",
                "  - from: \".\"\n",
                "    to: \"{}\"\n",
                "    configs:\n",
                "      - file: .profile\n"
            ),
            repository.path().display(),
            home.path().display()
        );
        let config_path = home.path().join("gitenv.yml");
        fs::write(&config_path, config).expect("config file should be written");

        let env_values =
            BTreeMap::from([("HOME".to_string(), OsString::from(home.path().as_os_str()))]);
        let get_env_var_os = move |name: &str| env_values.get(name).cloned();
        let output = run_apply(
            || load_config(&config_path),
            SystemCalls {
                get_env_var_os: &get_env_var_os,
            },
            RuntimeConfig::new(ColorMode::Auto, false, false),
        )
        .expect("run_apply_from_env should succeed with injected config path");

        let expected = format!(
            "created symlink ~/.profile -> {}",
            repository.path().join(".").join(".profile").display()
        );
        assert_eq!(output.message, expected);
    }

    #[test]
    fn propagate_config_loader_errors_from_apply_wrapper() {
        let get_env_var_os = |_: &str| None::<OsString>;
        let output = run_apply(
            || Err(ProgramError::HomeDirectoryUnavailable),
            SystemCalls {
                get_env_var_os: &get_env_var_os,
            },
            RuntimeConfig::new(ColorMode::Auto, false, false),
        )
        .expect_err("apply wrapper should propagate path resolution errors");

        assert_eq!(output, ProgramError::HomeDirectoryUnavailable);
    }

    #[test]
    fn cannot_load_default_config_without_home_or_override() {
        let get_env_var_os = |_: &str| None::<OsString>;
        let error = load_default_config_with_system(SystemCalls {
            get_env_var_os: &get_env_var_os,
        })
        .expect_err("default config loading should fail without HOME and override");

        assert_eq!(error, ProgramError::HomeDirectoryUnavailable);
    }

    #[test]
    fn load_default_config_when_home_is_available() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        let config_path = home
            .path()
            .join(".config")
            .join("gitenv")
            .join("config.yml");

        fs::create_dir_all(
            config_path
                .parent()
                .expect("default config path should have a parent"),
        )
        .expect("default config directory should be created");
        fs::write(
            &config_path,
            format!(
                concat!(
                    "version: 1\n",
                    "repository: \"{}\"\n",
                    "sources:\n",
                    "  - from: \".\"\n",
                    "    configs:\n",
                    "      - file: .zshrc\n"
                ),
                repository.path().display()
            ),
        )
        .expect("default config file should be written");

        let env_values =
            BTreeMap::from([("HOME".to_string(), OsString::from(home.path().as_os_str()))]);
        let get_env_var_os = move |name: &str| env_values.get(name).cloned();
        let config = load_default_config_with_system(SystemCalls {
            get_env_var_os: &get_env_var_os,
        })
        .expect("default config loading should succeed with HOME");

        assert_eq!(config.repository, repository.path().display().to_string());
    }

    #[test]
    fn format_program_errors_with_stable_messages() {
        let include_paths = vec![
            PathBuf::from("/includes/shared.yml"),
            PathBuf::from("/includes/private.yml"),
        ];
        let include_cycle = vec![
            PathBuf::from("/includes/root.yml"),
            PathBuf::from("/includes/child.yml"),
            PathBuf::from("/includes/root.yml"),
        ];

        assert_eq!(
            ProgramError::InvalidConfiguration {
                message: "bad yaml".to_string(),
            }
            .to_string(),
            "configuration is invalid (bad yaml)"
        );
        assert_eq!(
            ProgramError::ReadConfiguration {
                path: PathBuf::from("/home/.config/gitenv/config.yml"),
                message: "missing file".to_string(),
            }
            .to_string(),
            concat!(
                "cannot read config at /home/.config/gitenv/config.yml (missing file)\n\n",
                "Config file locations\n",
                "  - $GITENV_CONFIG (if set)\n",
                "  - ${XDG_CONFIG_HOME}/gitenv/config.yml (if $XDG_CONFIG_HOME is set)\n",
                "  - ${HOME}/.config/gitenv/config.yml (default fallback)",
            )
        );
        assert_eq!(
            ProgramError::MissingEnvironment {
                vars: vec!["PRIVATE_ENV_DIR".to_string(), "OTHER".to_string()],
            }
            .to_string(),
            "required environment variables are missing (PRIVATE_ENV_DIR, OTHER)"
        );
        assert_eq!(
            ProgramError::IncludeNotFound {
                paths: include_paths,
            }
            .to_string(),
            concat!(
                "required include files are missing\n",
                "  - /includes/shared.yml\n",
                "  - /includes/private.yml"
            )
        );
        assert_eq!(
            ProgramError::IncludeCycle {
                cycle: include_cycle,
            }
            .to_string(),
            concat!(
                "include cycle is detected\n",
                "  - /includes/root.yml\n",
                "  - /includes/child.yml\n",
                "  - /includes/root.yml"
            )
        );
        assert_eq!(
            ProgramError::ReadSourceDirectory {
                path: PathBuf::from("/repo/private"),
                message: "permission denied".to_string(),
            }
            .to_string(),
            "cannot read source directory /repo/private (permission denied)"
        );
        assert_eq!(
            ProgramError::InspectPathMetadata {
                path: PathBuf::from("/home/.zshrc"),
                message: "input/output error".to_string(),
            }
            .to_string(),
            "cannot inspect path metadata for /home/.zshrc (input/output error)"
        );
        assert_eq!(
            ProgramError::ReadSymlinkTarget {
                path: PathBuf::from("/home/.gitconfig"),
                message: "broken link".to_string(),
            }
            .to_string(),
            "cannot read symlink target for /home/.gitconfig (broken link)"
        );
        assert_eq!(
            ProgramError::CreateSymlink {
                source: PathBuf::from("/repo/.gitconfig"),
                target: PathBuf::from("/home/.gitconfig"),
                message: "operation not permitted".to_string(),
            }
            .to_string(),
            "cannot create symlink /home/.gitconfig -> /repo/.gitconfig (operation not permitted)"
        );
        assert_eq!(
            ProgramError::CopyFile {
                source: PathBuf::from("/repo/.gitconfig"),
                target: PathBuf::from("/home/.gitconfig"),
                message: "permission denied".to_string(),
            }
            .to_string(),
            "cannot copy file /repo/.gitconfig -> /home/.gitconfig (permission denied)"
        );
        assert_eq!(
            ProgramError::CreateTargetDirectory {
                path: PathBuf::from("/home/.config"),
                message: "permission denied".to_string(),
            }
            .to_string(),
            "cannot create target directory /home/.config (permission denied)"
        );
        assert_eq!(
            ProgramError::RemoveTarget {
                path: PathBuf::from("/home/.gitconfig"),
                message: "permission denied".to_string(),
            }
            .to_string(),
            "cannot remove existing target /home/.gitconfig (permission denied)"
        );
        assert_eq!(
            ProgramError::BackupTarget {
                path: PathBuf::from("/home/.gitconfig"),
                backup_path: PathBuf::from("/home/.gitconfig.orig"),
                message: "permission denied".to_string(),
            }
            .to_string(),
            "cannot move existing target /home/.gitconfig to backup /home/.gitconfig.orig (permission denied)"
        );
        assert_eq!(
            ProgramError::BackupAlreadyExists {
                path: PathBuf::from("/home/.gitconfig.orig"),
            }
            .to_string(),
            "cannot overwrite because backup already exists at /home/.gitconfig.orig"
        );
        assert_eq!(
            ProgramError::HomeDirectoryUnavailable.to_string(),
            "cannot resolve home directory from $HOME"
        );
    }
}
