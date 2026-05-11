//! Orchestration for the `info` command.
//!
//! Drives the flow from a pre-loaded config through intent planning, operation
//! planning, and CLI rendering. Boundary wiring lives in the composition root
//! (`lib.rs`); this module only coordinates already-wired stages.

use crate::boundary::{ConfigReader, DirectoryProbe};
use crate::{
    LoadedConfig, ProgramError, ProgramOutput, RuntimeConfig, SystemCalls, boundary, cli,
    derive_intent_plan_with_injectables, derive_operation_plan, fs_adapter,
};

/// Runs the `info` command: derives the intent and operation plans from
/// `loaded_config`, then returns rendered inspection output.
pub(crate) fn run_info(
    load_config: impl FnOnce() -> Result<LoadedConfig, ProgramError>,
    system: SystemCalls<'_>,
    runtime_config: RuntimeConfig,
) -> Result<ProgramOutput, ProgramError> {
    let loaded_config = load_config()?;
    let home_directory = fs_adapter::resolve_home_directory(system.get_env_var)?;
    let boundary = boundary::RealBoundary;
    let intent_plan = derive_intent_plan_with_injectables(
        &loaded_config,
        system.get_env_var,
        &home_directory,
        &|path| boundary.is_directory(path),
        &|path| boundary.read_config_file(path),
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

#[cfg(test)]
mod tests {
    use super::run_info;
    use crate::{
        ActionMode, ColorMode, Config, ConfigItem, Defaults, FileConfig, LoadedConfig,
        ProgramError, RuntimeConfig, SelectConfig, Source, SourceRoot, SystemCalls, load_config,
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

    fn loaded_config_for_test(config: Config) -> LoadedConfig {
        LoadedConfig {
            path: PathBuf::from("/tmp/gitenv-test-config.yml"),
            config,
        }
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
}
