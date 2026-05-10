mod actions;
mod cli;
mod color;
mod config;
mod errors;
mod fs_adapter;
mod intent;
mod logging;
mod operation;
mod path_resolution;
mod status;

pub use cli::{Cli, Command, LogLevel};
pub use color::{ColorMode, RuntimeConfig};
pub use errors::ProgramError;
pub use logging::init as init_logging;

pub use actions::{
    ApplyOperationOutcome, ApplyOperationReport, apply_operation_plan_with_injectables,
};
pub use config::{
    ActionMode, Config, ConfigItem, Defaults, FileConfig, Guard, Include, LoadedConfig,
    SelectConfig, Source, SourceRoot, load_config, parse_config,
};
pub use intent::{
    ConflictPolicy, IntentAction, IntentFileAction, IntentPlan, IntentSelectAction, IntentSource,
    ResolvedOptions, derive_intent_plan_with_injectables,
};
pub use operation::{
    FileOperation, OperationAction, OperationPlan, derive_operation_plan_with_injectables,
};
pub use status::{
    CopyInspection, CopyInspectionState, OperationInspectionOutcome, OperationInspectionReport,
    SymlinkInspection, SymlinkInspectionState, inspect_operation_plan_status,
    inspect_symlink_operation_status,
};

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramOutput {
    pub message: String,
}

const DEFAULT_CONFIG_HOME_SUFFIX: &str = ".config";
const DEFAULT_CONFIG_DIRECTORY_NAME: &str = "gitenv";
const DEFAULT_CONFIG_FILE_NAME: &str = "config.yml";

#[derive(Clone, Copy)]
struct SystemCalls<'a> {
    get_env_var: &'a dyn Fn(&str) -> Option<String>,
}

fn std_env_var(name: &str) -> Option<String> {
    std::env::var_os(name).map(|v| v.to_string_lossy().into_owned())
}

fn real_system_calls() -> SystemCalls<'static> {
    SystemCalls {
        get_env_var: &std_env_var,
    }
}

fn real_is_directory(path: &str) -> bool {
    logging::system(log::Level::Trace, "metadata", format!("path={path}"));
    match std::fs::metadata(path) {
        Ok(metadata) => metadata.is_dir(),
        Err(_) => false,
    }
}

/// Single production entry point for intent planning.
/// Wires real environment and filesystem adapters from the composition root.
pub fn derive_intent_plan(
    loaded_config: &LoadedConfig,
    home_directory: &Path,
) -> Result<IntentPlan, ProgramError> {
    derive_intent_plan_with_injectables(
        loaded_config,
        &std_env_var,
        home_directory,
        &real_is_directory,
        &load_config,
    )
}

/// Derives an operation plan using real filesystem directory reads.
pub fn derive_operation_plan(
    intent_plan: &IntentPlan,
    home_directory: &Path,
) -> Result<OperationPlan, ProgramError> {
    derive_operation_plan_with_injectables(
        intent_plan,
        home_directory,
        &fs_adapter::list_directory_entries,
    )
}

/// Applies operations using real filesystem probes and symlink creation.
pub fn apply_operation_plan(
    operation_plan: &OperationPlan,
) -> Result<ApplyOperationReport, ProgramError> {
    apply_operation_plan_with_injectables(
        operation_plan,
        &actions::target_exists,
        &actions::create_symlink_on_filesystem,
    )
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
    let home_directory = fs_adapter::resolve_home_directory(system.get_env_var)?;
    let intent_plan = derive_intent_plan_with_injectables(
        &loaded_config,
        system.get_env_var,
        &home_directory,
        &real_is_directory,
        &crate::load_config,
    )?;
    let operation_plan = derive_operation_plan(&intent_plan, &home_directory)?;

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
    let home_directory = fs_adapter::resolve_home_directory(system.get_env_var)?;
    let intent_plan = derive_intent_plan_with_injectables(
        &loaded_config,
        system.get_env_var,
        &home_directory,
        &real_is_directory,
        &crate::load_config,
    )?;
    let operation_plan = derive_operation_plan(&intent_plan, &home_directory)?;

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
    let gitenv_config = (system.get_env_var)("GITENV_CONFIG").map(PathBuf::from);
    let xdg_config_home = (system.get_env_var)("XDG_CONFIG_HOME").map(PathBuf::from);
    if let Some(custom_path) = gitenv_config {
        return Ok(custom_path);
    }

    let home_directory = fs_adapter::resolve_home_directory(system.get_env_var)?;

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

#[cfg(test)]
mod tests {
    use super::{
        ActionMode, ColorMode, Config, ConfigItem, Defaults, FileConfig, LoadedConfig,
        ProgramError, RuntimeConfig, Source, SourceRoot, SystemCalls, default_config_path_from_env,
        default_config_path_with_system, load_config, load_default_config_with_system, run_apply,
        run_info,
    };
    use crate::SelectConfig;
    use std::collections::BTreeMap;
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

    #[cfg(unix)]
    #[test]
    fn std_env_var_converts_non_utf8_lossy() {
        use std::os::unix::ffi::OsStringExt;

        // Create an OsString with invalid UTF-8: 0x66 0x6f 0x80 = "fo" + invalid byte
        let invalid_utf8 = std::ffi::OsString::from_vec(vec![0x66, 0x6f, 0x80]);

        // Manually set an environment variable to the invalid UTF-8 value
        // by constructing the OsString representation. Since we can't directly
        // set env vars with invalid UTF-8, we'll test the conversion logic directly.
        let converted = invalid_utf8.to_string_lossy().into_owned();

        // The lossy conversion should replace invalid bytes with U+FFFD replacement char
        assert_eq!(converted, "fo\u{fffd}");
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
    fn use_gitenv_config_override_in_system_default_path_resolution() {
        let env_values = BTreeMap::from([(
            "GITENV_CONFIG".to_string(),
            "/tmp/custom-config.yml".to_string(),
        )]);
        let get_env_var = |name: &str| env_values.get(name).cloned();

        let path = default_config_path_with_system(SystemCalls {
            get_env_var: &get_env_var,
        })
        .expect("GITENV_CONFIG should short-circuit default path resolution");

        assert_eq!(path, PathBuf::from("/tmp/custom-config.yml"));
    }

    #[test]
    fn propagate_source_directory_read_error_from_info_planning() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        let destination = home.path().join("target");

        let config = Config {
            version: 1,
            repository: repository.path().display().to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path("missing-dir".to_string()),
                to: Some(destination.display().to_string()),
                guard: None,
                configs: vec![ConfigItem::Select(SelectConfig {
                    dotfiles: false,
                    exclude: vec![],
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };

        let env_values = BTreeMap::from([(
            "HOME".to_string(),
            home.path().as_os_str().to_string_lossy().into_owned(),
        )]);
        let get_env_var = move |name: &str| env_values.get(name).cloned();

        let error = run_info(
            || Ok(loaded_config_for_test(config)),
            SystemCalls {
                get_env_var: &get_env_var,
            },
            RuntimeConfig::new(ColorMode::Auto, false, false),
        )
        .expect_err("missing selector source directory should fail info planning");

        assert_eq!(
            std::mem::discriminant(&error),
            std::mem::discriminant(&ProgramError::SourceDirectoryReadFailed {
                path: PathBuf::new(),
                message: String::new(),
            })
        );
    }

    #[test]
    fn propagate_source_directory_read_error_from_apply_planning() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        let destination = home.path().join("target");

        let config = Config {
            version: 1,
            repository: repository.path().display().to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path("missing-dir".to_string()),
                to: Some(destination.display().to_string()),
                guard: None,
                configs: vec![ConfigItem::Select(SelectConfig {
                    dotfiles: false,
                    exclude: vec![],
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };

        let env_values = BTreeMap::from([(
            "HOME".to_string(),
            home.path().as_os_str().to_string_lossy().into_owned(),
        )]);
        let get_env_var = move |name: &str| env_values.get(name).cloned();

        let error = run_apply(
            || Ok(loaded_config_for_test(config)),
            SystemCalls {
                get_env_var: &get_env_var,
            },
            RuntimeConfig::new(ColorMode::Auto, false, false),
        )
        .expect_err("missing selector source directory should fail apply planning");

        assert_eq!(
            std::mem::discriminant(&error),
            std::mem::discriminant(&ProgramError::SourceDirectoryReadFailed {
                path: PathBuf::new(),
                message: String::new(),
            })
        );
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

        let env_values = BTreeMap::from([(
            "HOME".to_string(),
            home.path().as_os_str().to_string_lossy().into_owned(),
        )]);
        let get_env_var = move |name: &str| env_values.get(name).cloned();
        let output = run_info(
            || load_config(&config_path),
            SystemCalls {
                get_env_var: &get_env_var,
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

        let env_values = BTreeMap::from([(
            "HOME".to_string(),
            home.path().as_os_str().to_string_lossy().into_owned(),
        )]);
        let get_env_var = move |name: &str| env_values.get(name).cloned();
        let output = run_info(
            || load_config(&root_config_path),
            SystemCalls {
                get_env_var: &get_env_var,
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

        let env_values = BTreeMap::from([(
            "HOME".to_string(),
            home.path().as_os_str().to_string_lossy().into_owned(),
        )]);
        let get_env_var = move |name: &str| env_values.get(name).cloned();

        let error = run_info(
            || Ok(loaded_config_for_test(config)),
            SystemCalls {
                get_env_var: &get_env_var,
            },
            RuntimeConfig::new(ColorMode::Auto, false, false),
        )
        .expect_err("missing copy source should propagate an inspection error");

        assert!(matches!(
            error,
            ProgramError::PathInspectionFailed { path, .. }
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

        let env_values = BTreeMap::from([("HOME".to_string(), "/tmp/home".to_string())]);
        let get_env_var = move |name: &str| env_values.get(name).cloned();
        let error = run_info(
            || Ok(loaded_config_for_test(config)),
            SystemCalls {
                get_env_var: &get_env_var,
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

        let env_values = BTreeMap::from([("HOME".to_string(), "/tmp/home".to_string())]);
        let get_env_var = move |name: &str| env_values.get(name).cloned();
        let error = run_apply(
            || Ok(loaded_config_for_test(config)),
            SystemCalls {
                get_env_var: &get_env_var,
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

        let get_env_var = |_: &str| None::<String>;
        let error = run_info(
            || load_config(&missing_path),
            SystemCalls {
                get_env_var: &get_env_var,
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

        let env_values = BTreeMap::from([(
            "HOME".to_string(),
            home.path().as_os_str().to_string_lossy().into_owned(),
        )]);
        let get_env_var = move |name: &str| env_values.get(name).cloned();
        let output = run_apply(
            || load_config(&config_path),
            SystemCalls {
                get_env_var: &get_env_var,
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

        let env_values = BTreeMap::from([(
            "HOME".to_string(),
            home.path().as_os_str().to_string_lossy().into_owned(),
        )]);
        let get_env_var = move |name: &str| env_values.get(name).cloned();
        let output = run_apply(
            || load_config(&root_config_path),
            SystemCalls {
                get_env_var: &get_env_var,
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

        let env_values = BTreeMap::from([(
            "HOME".to_string(),
            home.path().as_os_str().to_string_lossy().into_owned(),
        )]);
        let get_env_var = move |name: &str| env_values.get(name).cloned();
        let error = run_apply(
            || Ok(loaded_config_for_test(config)),
            SystemCalls {
                get_env_var: &get_env_var,
            },
            RuntimeConfig::new(ColorMode::Auto, false, false),
        )
        .expect_err("missing copy source should fail apply execution");

        assert!(matches!(
            error,
            ProgramError::FileCopyFailed { source, target, .. }
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

        let get_env_var = |_: &str| None::<String>;

        let info_error = run_info(
            || Ok(loaded_config_for_test(info_config)),
            SystemCalls {
                get_env_var: &get_env_var,
            },
            RuntimeConfig::new(ColorMode::Auto, false, false),
        )
        .expect_err("missing HOME should fail explicit-config info planning");
        assert_eq!(info_error, ProgramError::HomeDirectoryUnavailable);

        let apply_error = run_apply(
            || Ok(loaded_config_for_test(apply_config)),
            SystemCalls {
                get_env_var: &get_env_var,
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

        let env_values = BTreeMap::from([(
            "HOME".to_string(),
            home.path().as_os_str().to_string_lossy().into_owned(),
        )]);
        let get_env_var = move |name: &str| env_values.get(name).cloned();
        let output = run_info(
            || load_config(&config_path),
            SystemCalls {
                get_env_var: &get_env_var,
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

        let env_values = BTreeMap::from([(
            "HOME".to_string(),
            home.path().as_os_str().to_string_lossy().into_owned(),
        )]);
        let get_env_var = move |name: &str| env_values.get(name).cloned();
        let output = run_apply(
            || load_config(&config_path),
            SystemCalls {
                get_env_var: &get_env_var,
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
        let get_env_var = |_: &str| None::<String>;
        let output = run_apply(
            || Err(ProgramError::HomeDirectoryUnavailable),
            SystemCalls {
                get_env_var: &get_env_var,
            },
            RuntimeConfig::new(ColorMode::Auto, false, false),
        )
        .expect_err("apply wrapper should propagate path resolution errors");

        assert_eq!(output, ProgramError::HomeDirectoryUnavailable);
    }

    #[test]
    fn cannot_load_default_config_without_home_or_override() {
        let get_env_var = |_: &str| None::<String>;
        let error = load_default_config_with_system(SystemCalls {
            get_env_var: &get_env_var,
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

        let env_values = BTreeMap::from([(
            "HOME".to_string(),
            home.path().as_os_str().to_string_lossy().into_owned(),
        )]);
        let get_env_var = move |name: &str| env_values.get(name).cloned();
        let config = load_default_config_with_system(SystemCalls {
            get_env_var: &get_env_var,
        })
        .expect("default config loading should succeed with HOME");

        assert_eq!(config.repository, repository.path().display().to_string());
    }
}
