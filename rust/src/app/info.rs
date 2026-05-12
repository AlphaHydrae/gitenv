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
    runtime_config: RuntimeConfig,
    context: InfoCommandContext,
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
    use crate::{
        ActionMode, ColorMode, Config, ConfigItem, ConflictPolicy, Defaults, FileConfig,
        FileOperation, Guard, Include, IntentAction, IntentFileAction, IntentPlan, IntentSource,
        LoadedConfig, OperationAction, OperationPlan, ProgramError, ResolvedOptions, RuntimeConfig,
        Source, SourceRoot,
    };
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

    /// Helper to run info with fake planning stages, used by most tests.
    /// Fake planners return pre-built test data, so tests verify orchestration
    /// without duplicating lower-layer planning coverage.
    fn run_info_with(
        home_directory: PathBuf,
        loaded_config: LoadedConfig,
        runtime_config: RuntimeConfig,
        intent_plan: Result<IntentPlan, ProgramError>,
        operation_plan: Result<OperationPlan, ProgramError>,
    ) -> Result<crate::ProgramOutput, ProgramError> {
        run_info(
            runtime_config,
            InfoCommandContext::new(
                loaded_config,
                home_directory,
                Box::new(move |_, _| intent_plan.clone()),
                Box::new(move |_, _| operation_plan.clone()),
            ),
        )
    }

    #[test]
    fn show_info_output_when_directory_guard_is_satisfied() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let home_path = home.path().to_path_buf();

        let intent_plan = IntentPlan {
            repository: "/repo".to_string(),
            sources: vec![IntentSource {
                from: ".".to_string(),
                actions: vec![IntentAction::File(IntentFileAction {
                    file: "guarded.conf".to_string(),
                    as_name: "guarded.conf".to_string(),
                    options: ResolvedOptions {
                        mode: ActionMode::Symlink,
                        to: "~".to_string(),
                        mkdir: true,
                        conflict_policy: ConflictPolicy::Skip,
                    },
                })],
            }],
        };

        let operation_plan = OperationPlan {
            actions: vec![OperationAction::Symlink(FileOperation {
                source: PathBuf::from("/repo/guarded.conf"),
                target: home_path.join("guarded.conf"),
                mkdir: true,
                conflict_policy: ConflictPolicy::Skip,
            })],
        };

        let output = run_info_with(
            home_path.clone(),
            make_loaded_config(make_config(
                Path::new("/repo"),
                vec![Source {
                    from: SourceRoot::Path(".".to_string()),
                    to: Some(home_path.display().to_string()),
                    guard: Some(Guard::ToExists),
                    configs: vec![make_file_config("guarded.conf", ActionMode::Symlink)],
                }],
            )),
            RuntimeConfig::new(ColorMode::Auto, false, false),
            Ok(intent_plan),
            Ok(operation_plan),
        )
        .expect("orchestration should run with fake stages");

        assert_eq!(
            output.message,
            format!(
                "~/{} -> /repo/guarded.conf   not yet set up",
                "guarded.conf"
            )
        );
    }

    #[test]
    fn cannot_run_info_when_a_source_directory_cannot_be_read() {
        let home_path = PathBuf::from("/tmp/home");

        let error = run_info_with(
            home_path,
            make_loaded_config(Config {
                version: 1,
                repository: "/repo".to_string(),
                defaults: Defaults::default(),
                includes: vec![],
                sources: vec![],
            }),
            RuntimeConfig::new(ColorMode::Auto, false, false),
            Err(ProgramError::SourceDirectoryReadFailed {
                path: PathBuf::from("missing-dir"),
                message: "not found".to_string(),
            }),
            Ok(OperationPlan { actions: vec![] }),
        )
        .expect_err("error from intent planner should propagate");

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
        let home_path = PathBuf::from("/tmp/home");

        let intent_plan = IntentPlan {
            repository: "/repo".to_string(),
            sources: vec![IntentSource {
                from: ".".to_string(),
                actions: vec![IntentAction::File(IntentFileAction {
                    file: ".gitconfig".to_string(),
                    as_name: ".gitconfig".to_string(),
                    options: ResolvedOptions {
                        mode: ActionMode::Symlink,
                        to: "~".to_string(),
                        mkdir: true,
                        conflict_policy: ConflictPolicy::Skip,
                    },
                })],
            }],
        };

        let operation_plan = OperationPlan {
            actions: vec![OperationAction::Symlink(FileOperation {
                source: PathBuf::from("/repo/.gitconfig"),
                target: home_path.join(".gitconfig"),
                mkdir: true,
                conflict_policy: ConflictPolicy::Skip,
            })],
        };

        let output = run_info_with(
            home_path.clone(),
            make_loaded_config(make_config(
                Path::new("/repo"),
                vec![make_source(
                    &home_path,
                    vec![make_file_config(".gitconfig", ActionMode::Symlink)],
                )],
            )),
            RuntimeConfig::new(ColorMode::Auto, false, false),
            Ok(intent_plan),
            Ok(operation_plan),
        )
        .expect("orchestration should run with fake stages");

        assert_eq!(
            output.message,
            format!("~/.gitconfig -> /repo/.gitconfig   not yet set up")
        );
    }

    #[test]
    fn show_default_inspection_output_when_config_path_is_provided_for_relative_include() {
        let home_path = PathBuf::from("/tmp/home");

        let intent_plan = IntentPlan {
            repository: "/repo".to_string(),
            sources: vec![
                IntentSource {
                    from: ".".to_string(),
                    actions: vec![IntentAction::File(IntentFileAction {
                        file: "root.conf".to_string(),
                        as_name: ".root.conf".to_string(),
                        options: ResolvedOptions {
                            mode: ActionMode::Symlink,
                            to: "~".to_string(),
                            mkdir: true,
                            conflict_policy: ConflictPolicy::Skip,
                        },
                    })],
                },
                IntentSource {
                    from: ".".to_string(),
                    actions: vec![IntentAction::File(IntentFileAction {
                        file: "shared.conf".to_string(),
                        as_name: ".shared.conf".to_string(),
                        options: ResolvedOptions {
                            mode: ActionMode::Symlink,
                            to: "~".to_string(),
                            mkdir: true,
                            conflict_policy: ConflictPolicy::Skip,
                        },
                    })],
                },
            ],
        };

        let operation_plan = OperationPlan {
            actions: vec![
                OperationAction::Symlink(FileOperation {
                    source: PathBuf::from("/repo/root.conf"),
                    target: home_path.join(".root.conf"),
                    mkdir: true,
                    conflict_policy: ConflictPolicy::Skip,
                }),
                OperationAction::Symlink(FileOperation {
                    source: PathBuf::from("/repo/shared.conf"),
                    target: home_path.join(".shared.conf"),
                    mkdir: true,
                    conflict_policy: ConflictPolicy::Skip,
                }),
            ],
        };

        let loaded_config = LoadedConfig {
            path: PathBuf::from("/home/configs/root.yml"),
            config: Config {
                version: 1,
                repository: "/repo".to_string(),
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

        let output = run_info_with(
            home_path,
            loaded_config,
            RuntimeConfig::new(ColorMode::Auto, false, false),
            Ok(intent_plan),
            Ok(operation_plan),
        )
        .expect("orchestration should run with fake stages");

        let expected = concat!(
            "~/.root.conf -> /repo/root.conf   not yet set up\n",
            "~/.shared.conf -> /repo/shared.conf   not yet set up"
        );
        assert_eq!(output.message, expected);
    }

    #[test]
    fn cannot_run_info_when_copy_source_cannot_be_read() {
        let home_path = PathBuf::from("/tmp/home");

        let intent_plan = IntentPlan {
            repository: "/repo".to_string(),
            sources: vec![IntentSource {
                from: ".".to_string(),
                actions: vec![IntentAction::File(IntentFileAction {
                    file: "missing.txt".to_string(),
                    as_name: "missing.txt".to_string(),
                    options: ResolvedOptions {
                        mode: ActionMode::Copy,
                        to: "~".to_string(),
                        mkdir: true,
                        conflict_policy: ConflictPolicy::Skip,
                    },
                })],
            }],
        };

        let operation_plan = OperationPlan {
            actions: vec![OperationAction::Copy(FileOperation {
                source: PathBuf::from("/repo/missing.txt"),
                target: home_path.join("missing.txt"),
                mkdir: true,
                conflict_policy: ConflictPolicy::Skip,
            })],
        };

        let output = run_info_with(
            home_path,
            make_loaded_config(Config {
                version: 1,
                repository: "/repo".to_string(),
                defaults: Defaults::default(),
                includes: vec![],
                sources: vec![],
            }),
            RuntimeConfig::new(ColorMode::Auto, false, false),
            Ok(intent_plan),
            Ok(operation_plan),
        )
        .expect("orchestration should run with fake stages and produce output");

        // With fake stages, source missing errors only occur during apply execution,
        // not during inspection (which is tested in actions tests).
        assert!(output.message.contains("missing.txt"));
    }

    #[test]
    fn cannot_run_info_when_status_inspection_fails() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let home_path = home.path().to_path_buf();
        let target_file = home.path().join("target-file");
        std::fs::write(&target_file, "present\n").expect("target file should be written");
        let missing_source = home.path().join("missing-source");

        let intent_plan = IntentPlan {
            repository: "/repo".to_string(),
            sources: vec![],
        };

        let operation_plan = OperationPlan {
            actions: vec![OperationAction::Copy(FileOperation {
                source: missing_source.clone(),
                target: target_file,
                mkdir: false,
                conflict_policy: ConflictPolicy::Skip,
            })],
        };

        let error = run_info_with(
            home_path,
            make_loaded_config(make_config(home.path(), vec![])),
            RuntimeConfig::new(ColorMode::No, true, true),
            Ok(intent_plan),
            Ok(operation_plan),
        )
        .expect_err("status inspection errors should propagate from rendering");

        assert!(matches!(
            error,
            ProgramError::PathInspectionFailed { path, .. } if path == missing_source
        ));
    }

    #[test]
    fn cannot_run_info_when_source_environment_is_missing() {
        let home_path = PathBuf::from("/tmp/home");

        let error = run_info_with(
            home_path,
            make_loaded_config(Config {
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
            }),
            RuntimeConfig::new(ColorMode::Auto, false, false),
            Err(ProgramError::MissingEnvironment {
                vars: vec!["MISSING_SOURCE_ROOT".to_string()],
            }),
            Ok(OperationPlan { actions: vec![] }),
        )
        .expect_err("error from intent planner should propagate");

        assert_eq!(
            error,
            ProgramError::MissingEnvironment {
                vars: vec!["MISSING_SOURCE_ROOT".to_string()],
            }
        );
    }
}
