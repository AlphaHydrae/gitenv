//! Orchestration for the `info` command.
//!
//! Drives the flow from a pre-loaded config through intent planning, operation
//! planning, and CLI rendering. The composition root (`lib.rs`) resolves
//! runtime facts and wires stage entrypoints into `InfoCommandContext`.

use crate::{
    IntentPlan, LoadedConfig, OperationPlan, ProgramError, ProgramOutput, RuntimeConfig, cli,
};
use std::path::{Path, PathBuf};

type IntentPlanner = dyn Fn(&LoadedConfig, &Path) -> Result<IntentPlan, ProgramError>;
type OperationPlanner = dyn Fn(&IntentPlan, &Path) -> Result<OperationPlan, ProgramError>;

/// Command-owned context for `info` orchestration.
pub(crate) struct InfoCommandContext {
    pub(crate) loaded_config: LoadedConfig,
    pub(crate) home_directory: PathBuf,
    pub(crate) derive_intent_plan: Box<IntentPlanner>,
    pub(crate) derive_operation_plan: Box<OperationPlanner>,
}

impl InfoCommandContext {
    pub(crate) fn new(
        loaded_config: LoadedConfig,
        home_directory: PathBuf,
        derive_intent_plan: Box<IntentPlanner>,
        derive_operation_plan: Box<OperationPlanner>,
    ) -> Self {
        Self {
            loaded_config,
            home_directory,
            derive_intent_plan,
            derive_operation_plan,
        }
    }
}

/// Runs the `info` command with a pre-wired command context.
pub(crate) fn run_info(
    context: InfoCommandContext,
    runtime_config: RuntimeConfig,
) -> Result<ProgramOutput, ProgramError> {
    let intent_plan =
        (context.derive_intent_plan)(&context.loaded_config, &context.home_directory)?;
    let operation_plan = (context.derive_operation_plan)(&intent_plan, &context.home_directory)?;
    Ok(ProgramOutput {
        message: cli::render_default_inspection_output(
            &operation_plan,
            &context.home_directory,
            runtime_config.use_color_for_stdout,
        )?,
    })
}

#[cfg(test)]
mod tests {
    use super::{InfoCommandContext, run_info};
    use crate::boundary::{ConfigReader, DirectoryProbe};
    use crate::{
        ActionMode, ColorMode, Config, ConfigItem, Defaults, FileConfig, Guard, Include,
        LoadedConfig, ProgramError, RuntimeConfig, SelectConfig, Source, SourceRoot,
        derive_intent_plan_with_injectables, derive_operation_plan, fs_adapter,
    };
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

    fn make_loaded_config(config: Config) -> LoadedConfig {
        LoadedConfig {
            path: PathBuf::from("/tmp/gitenv-test-config.yml"),
            config,
        }
    }

    fn run_info_with(
        loaded_config: LoadedConfig,
        env_values: BTreeMap<String, String>,
        runtime_config: RuntimeConfig,
    ) -> Result<crate::ProgramOutput, ProgramError> {
        let get_env_var = move |name: &str| env_values.get(name).cloned();
        let home_directory = fs_adapter::resolve_home_directory(&get_env_var)?;
        let boundary = crate::boundary::RealBoundary;
        run_info(
            InfoCommandContext::new(
                loaded_config,
                home_directory,
                Box::new(move |loaded_config, home_directory| {
                    derive_intent_plan_with_injectables(
                        loaded_config,
                        &get_env_var,
                        home_directory,
                        &|path| boundary.is_directory(path),
                        &|path| boundary.read_config_file(path),
                    )
                }),
                Box::new(derive_operation_plan),
            ),
            runtime_config,
        )
    }

    #[test]
    fn show_info_output_when_directory_guard_is_satisfied() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        fs::write(repository.path().join("guarded.conf"), "guarded\n")
            .expect("source file should be written");
        let destination = home.path().join("target");
        fs::create_dir_all(&destination).expect("guard destination should exist");

        let loaded_config = make_loaded_config(Config {
            version: 1,
            repository: repository.path().display().to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path(".".to_string()),
                to: Some(destination.display().to_string()),
                guard: Some(Guard::ToExists),
                configs: vec![make_file_config("guarded.conf", ActionMode::Symlink)],
            }],
        });

        let env_values = BTreeMap::from([(
            "HOME".to_string(),
            home.path().as_os_str().to_string_lossy().into_owned(),
        )]);
        let output = run_info_with(
            loaded_config,
            env_values,
            RuntimeConfig::new(ColorMode::Auto, false, false),
        )
        .expect("guarded source should appear in inspection when destination exists");

        assert_eq!(
            output.message,
            format!(
                "~/target/guarded.conf -> {}   not yet set up",
                repository.path().join(".").join("guarded.conf").display()
            )
        );
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
        let error = run_info_with(
            make_loaded_config(config),
            env_values,
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
    fn show_default_inspection_output_for_a_missing_symlink() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        fs::write(repository.path().join(".gitconfig"), "[user]\n")
            .expect("source file should be written");

        let loaded_config = make_loaded_config(make_config(
            repository.path(),
            vec![make_source(
                home.path(),
                vec![make_file_config(".gitconfig", ActionMode::Symlink)],
            )],
        ));

        let env_values = BTreeMap::from([(
            "HOME".to_string(),
            home.path().as_os_str().to_string_lossy().into_owned(),
        )]);
        let output = run_info_with(
            loaded_config,
            env_values,
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
        fs::write(&shared_config_path, shared_config).expect("shared config should be written");

        let loaded_config = LoadedConfig {
            path: root_config_path,
            config: Config {
                version: 1,
                repository: repository.path().display().to_string(),
                defaults: Defaults::default(),
                includes: vec![Include::Path {
                    path: "includes/shared.yml".to_string(),
                    optional: false,
                }],
                sources: vec![Source {
                    from: SourceRoot::Path(".".to_string()),
                    to: None,
                    guard: None,
                    configs: vec![ConfigItem::File(FileConfig {
                        file: "root.conf".to_string(),
                        as_name: Some(".root.conf".to_string()),
                        mode: None,
                        to: None,
                        mkdir: None,
                        overwrite: None,
                        backup_on_overwrite: None,
                    })],
                }],
            },
        };

        let env_values = BTreeMap::from([(
            "HOME".to_string(),
            home.path().as_os_str().to_string_lossy().into_owned(),
        )]);
        let output = run_info_with(
            loaded_config,
            env_values,
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
        let error = run_info_with(
            make_loaded_config(config),
            env_values,
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
        let error = run_info_with(
            make_loaded_config(config),
            env_values,
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
    fn show_default_inspection_output() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        fs::write(repository.path().join(".profile"), "# profile\n")
            .expect("source file should be written");

        let loaded_config = make_loaded_config(make_config(
            repository.path(),
            vec![make_source(
                home.path(),
                vec![make_file_config(".profile", ActionMode::Symlink)],
            )],
        ));

        let env_values = BTreeMap::from([(
            "HOME".to_string(),
            home.path().as_os_str().to_string_lossy().into_owned(),
        )]);
        let output = run_info_with(
            loaded_config,
            env_values,
            RuntimeConfig::new(ColorMode::Auto, false, false),
        )
        .expect("run_info_from_env should succeed with injected config path");

        let expected = format!(
            "~/.profile -> {}   not yet set up",
            repository.path().join(".").join(".profile").display()
        );
        assert_eq!(output.message, expected);
    }
}
