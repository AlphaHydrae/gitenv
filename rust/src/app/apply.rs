//! Orchestration for the `apply` command.
//!
//! Drives the flow from a pre-loaded config through intent planning, operation
//! planning, and apply execution, then returns rendered output. The
//! composition root (`lib.rs`) resolves runtime facts and wires stage
//! entrypoints into `ApplyCommandContext`.

use crate::{
    ApplyOperationReport, IntentPlan, LoadedConfig, OperationPlan, ProgramError, ProgramOutput,
    RuntimeConfig, cli,
};
use std::path::{Path, PathBuf};

type IntentPlanner = dyn Fn(&LoadedConfig, &Path) -> Result<IntentPlan, ProgramError>;
type OperationPlanner = dyn Fn(&IntentPlan, &Path) -> Result<OperationPlan, ProgramError>;
type ApplyExecutor = dyn Fn(&OperationPlan) -> Result<ApplyOperationReport, ProgramError>;

/// Command-owned context for `apply` orchestration.
pub(crate) struct ApplyCommandContext {
    pub(crate) loaded_config: LoadedConfig,
    pub(crate) home_directory: PathBuf,
    pub(crate) derive_intent_plan: Box<IntentPlanner>,
    pub(crate) derive_operation_plan: Box<OperationPlanner>,
    pub(crate) apply_operation_plan: Box<ApplyExecutor>,
}

impl ApplyCommandContext {
    pub(crate) fn new(
        loaded_config: LoadedConfig,
        home_directory: PathBuf,
        derive_intent_plan: Box<IntentPlanner>,
        derive_operation_plan: Box<OperationPlanner>,
        apply_operation_plan: Box<ApplyExecutor>,
    ) -> Self {
        Self {
            loaded_config,
            home_directory,
            derive_intent_plan,
            derive_operation_plan,
            apply_operation_plan,
        }
    }
}

/// Runs the `apply` command with a pre-wired command context.
pub(crate) fn run_apply(
    runtime_config: RuntimeConfig,
    context: ApplyCommandContext,
) -> Result<ProgramOutput, ProgramError> {
    let intent_plan =
        (context.derive_intent_plan)(&context.loaded_config, &context.home_directory)?;
    let operation_plan = (context.derive_operation_plan)(&intent_plan, &context.home_directory)?;
    let apply_report = (context.apply_operation_plan)(&operation_plan)?;
    let output_message = cli::render_apply_output(
        &apply_report,
        &context.home_directory,
        runtime_config.use_color_for_stdout,
    );
    Ok(ProgramOutput {
        message: output_message,
    })
}

#[cfg(test)]
mod tests {
    use super::{ApplyCommandContext, run_apply};
    use crate::{
        ActionMode, ApplyOperationOutcome, ApplyOperationReport, ColorMode, Config, ConfigItem,
        ConflictPolicy, Defaults, FileConfig, FileOperation, Guard, IntentAction, IntentFileAction,
        IntentPlan, IntentSource, LoadedConfig, OperationAction, OperationEntry, OperationPlan,
        PlannedOperationAction, ProgramError, ResolvedOptions, RuntimeConfig, Source,
        SourceAvailability, SourceRoot,
    };
    use std::path::{Path, PathBuf};
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

    fn available_operation_plan(actions: Vec<OperationAction>) -> OperationPlan {
        OperationPlan {
            entries: actions
                .into_iter()
                .map(|action| {
                    OperationEntry::Action(PlannedOperationAction {
                        action,
                        source_availability: SourceAvailability::Available,
                        skip_reason: None,
                    })
                })
                .collect(),
        }
    }

    /// Helper to run apply with fake planning stages. Fake planners return
    /// pre-built test data, so tests verify orchestration without duplicating
    /// lower-layer planning coverage.
    fn run_apply_with(
        home_directory: PathBuf,
        loaded_config: LoadedConfig,
        runtime_config: RuntimeConfig,
        intent_plan: Result<IntentPlan, ProgramError>,
        operation_plan: Result<OperationPlan, ProgramError>,
        apply_report: Result<ApplyOperationReport, ProgramError>,
    ) -> Result<crate::ProgramOutput, ProgramError> {
        run_apply(
            runtime_config,
            ApplyCommandContext::new(
                loaded_config,
                home_directory,
                Box::new(move |_, _| intent_plan.clone()),
                Box::new(move |_, _| operation_plan.clone()),
                Box::new(move |_| apply_report.clone()),
            ),
        )
    }

    #[test]
    fn show_apply_output_when_directory_guard_is_satisfied() {
        let home_path = PathBuf::from("/tmp/home");

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

        let operation_plan =
            available_operation_plan(vec![OperationAction::Symlink(FileOperation {
                source: PathBuf::from("/repo/guarded.conf"),
                target: home_path.join("guarded.conf"),
                mkdir: true,
                conflict_policy: ConflictPolicy::Skip,
            })]);

        let apply_report = ApplyOperationReport {
            outcomes: vec![ApplyOperationOutcome::Applied(OperationAction::Symlink(
                FileOperation {
                    source: PathBuf::from("/repo/guarded.conf"),
                    target: home_path.join("guarded.conf"),
                    mkdir: true,
                    conflict_policy: ConflictPolicy::Skip,
                },
            ))],
        };

        let output = run_apply_with(
            home_path.clone(),
            make_loaded_config(Config {
                version: 1,
                repository: "/repo".to_string(),
                defaults: Defaults::default(),
                includes: vec![],
                sources: vec![Source {
                    from: SourceRoot::Path(".".to_string()),
                    to: Some(home_path.display().to_string()),
                    guard: Some(Guard::ToExists),
                    configs: vec![make_file_config("guarded.conf", ActionMode::Symlink)],
                }],
            }),
            RuntimeConfig::new(ColorMode::Auto, false, false),
            Ok(intent_plan),
            Ok(operation_plan),
            Ok(apply_report),
        )
        .expect("orchestration should run with fake stages");

        assert!(output.message.contains("guarded.conf"));
    }

    #[test]
    fn cannot_apply_operations_when_source_directory_cannot_be_read_during_apply_planning() {
        let home_path = PathBuf::from("/tmp/home");

        let error = run_apply_with(
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
            Ok(available_operation_plan(vec![])),
            Ok(ApplyOperationReport { outcomes: vec![] }),
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
    fn cannot_run_apply_when_operation_planning_fails() {
        let home_path = PathBuf::from("/tmp/home");

        let error = run_apply_with(
            home_path,
            make_loaded_config(Config {
                version: 1,
                repository: "/repo".to_string(),
                defaults: Defaults::default(),
                includes: vec![],
                sources: vec![],
            }),
            RuntimeConfig::new(ColorMode::Auto, false, false),
            Ok(IntentPlan {
                repository: "/repo".to_string(),
                sources: vec![],
            }),
            Err(ProgramError::SourceDirectoryReadFailed {
                path: PathBuf::from("missing-operation-source"),
                message: "cannot read source".to_string(),
            }),
            Ok(ApplyOperationReport { outcomes: vec![] }),
        )
        .expect_err("error from operation planner should propagate");

        assert_eq!(
            error,
            ProgramError::SourceDirectoryReadFailed {
                path: PathBuf::from("missing-operation-source"),
                message: "cannot read source".to_string(),
            }
        );
    }

    #[test]
    fn cannot_run_apply_when_apply_execution_fails() {
        let home_path = PathBuf::from("/tmp/home");

        let error = run_apply_with(
            home_path,
            make_loaded_config(Config {
                version: 1,
                repository: "/repo".to_string(),
                defaults: Defaults::default(),
                includes: vec![],
                sources: vec![],
            }),
            RuntimeConfig::new(ColorMode::Auto, false, false),
            Ok(IntentPlan {
                repository: "/repo".to_string(),
                sources: vec![],
            }),
            Ok(available_operation_plan(vec![])),
            Err(ProgramError::OperationPlanBlocked {
                diagnostics: vec!["blocked by test".to_string()],
            }),
        )
        .expect_err("error from apply execution should propagate");

        assert_eq!(
            error,
            ProgramError::OperationPlanBlocked {
                diagnostics: vec!["blocked by test".to_string()],
            }
        );
    }

    #[test]
    fn show_apply_output_for_a_missing_symlink() {
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

        let operation_plan =
            available_operation_plan(vec![OperationAction::Symlink(FileOperation {
                source: PathBuf::from("/repo/.gitconfig"),
                target: home_path.join(".gitconfig"),
                mkdir: true,
                conflict_policy: ConflictPolicy::Skip,
            })]);

        let apply_report = ApplyOperationReport {
            outcomes: vec![ApplyOperationOutcome::Applied(OperationAction::Symlink(
                FileOperation {
                    source: PathBuf::from("/repo/.gitconfig"),
                    target: home_path.join(".gitconfig"),
                    mkdir: true,
                    conflict_policy: ConflictPolicy::Skip,
                },
            ))],
        };

        let output = run_apply_with(
            home_path,
            make_loaded_config(make_config(
                Path::new("/repo"),
                vec![make_source(
                    &PathBuf::from("/tmp/home"),
                    vec![make_file_config(".gitconfig", ActionMode::Symlink)],
                )],
            )),
            RuntimeConfig::new(ColorMode::Auto, false, false),
            Ok(intent_plan),
            Ok(operation_plan),
            Ok(apply_report),
        )
        .expect("orchestration should run with fake stages");

        assert!(output.message.contains(".gitconfig"));
    }

    #[test]
    fn cannot_run_apply_when_source_environment_is_missing() {
        let home_path = PathBuf::from("/tmp/home");

        let error = run_apply_with(
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
            Ok(available_operation_plan(vec![])),
            Ok(ApplyOperationReport { outcomes: vec![] }),
        )
        .expect_err("error from intent planner should propagate");

        assert_eq!(
            error,
            ProgramError::MissingEnvironment {
                vars: vec!["MISSING_SOURCE_ROOT".to_string()],
            }
        );
    }

    #[test]
    fn show_apply_output_when_operations_are_empty() {
        let home_path = PathBuf::from("/tmp/home");

        // Empty operation plan with successful stages
        let output = run_apply_with(
            home_path,
            make_loaded_config(Config {
                version: 1,
                repository: "/repo".to_string(),
                defaults: Defaults::default(),
                includes: vec![],
                sources: vec![],
            }),
            RuntimeConfig::new(ColorMode::Auto, false, false),
            Ok(IntentPlan {
                repository: "/repo".to_string(),
                sources: vec![],
            }),
            Ok(available_operation_plan(vec![])),
            Ok(ApplyOperationReport { outcomes: vec![] }),
        )
        .expect("orchestration should run with empty operations");

        assert_eq!(output.message, "No operations to apply.");
    }

    #[test]
    fn show_apply_output_for_a_loaded_config() {
        let home_path = PathBuf::from("/tmp/home");

        let intent_plan = IntentPlan {
            repository: "/repo".to_string(),
            sources: vec![IntentSource {
                from: ".".to_string(),
                actions: vec![IntentAction::File(IntentFileAction {
                    file: ".profile".to_string(),
                    as_name: ".profile".to_string(),
                    options: ResolvedOptions {
                        mode: ActionMode::Symlink,
                        to: "~".to_string(),
                        mkdir: true,
                        conflict_policy: ConflictPolicy::Skip,
                    },
                })],
            }],
        };

        let operation_plan =
            available_operation_plan(vec![OperationAction::Symlink(FileOperation {
                source: PathBuf::from("/repo/.profile"),
                target: home_path.join(".profile"),
                mkdir: true,
                conflict_policy: ConflictPolicy::Skip,
            })]);

        let apply_report = ApplyOperationReport {
            outcomes: vec![ApplyOperationOutcome::Applied(OperationAction::Symlink(
                FileOperation {
                    source: PathBuf::from("/repo/.profile"),
                    target: home_path.join(".profile"),
                    mkdir: true,
                    conflict_policy: ConflictPolicy::Skip,
                },
            ))],
        };

        let output = run_apply_with(
            home_path,
            make_loaded_config(make_config(
                Path::new("/repo"),
                vec![make_source(
                    &PathBuf::from("/tmp/home"),
                    vec![make_file_config(".profile", ActionMode::Symlink)],
                )],
            )),
            RuntimeConfig::new(ColorMode::Auto, false, false),
            Ok(intent_plan),
            Ok(operation_plan),
            Ok(apply_report),
        )
        .expect("orchestration should run with fake stages");

        assert!(output.message.contains(".profile"));
    }
}
