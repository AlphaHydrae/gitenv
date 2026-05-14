mod actions;
mod app;
mod boundary;
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

#[cfg(test)]
mod core_plan_snapshots_test;

pub use cli::{Cli, Command, LogLevel};
pub use color::{ColorMode, RuntimeConfig};
pub use errors::{ProgramError, SourceAvailability, SourceUnreadableKind};
pub use logging::init as init_logging;

pub use actions::{ApplyOperationOutcome, ApplyOperationReport};
pub use config::{
    ActionMode, Config, ConfigItem, Defaults, FileConfig, Guard, Include, LoadedConfig,
    SelectConfig, SelectionType, Source, SourceRoot, load_config, parse_config,
};
pub use intent::{
    ConflictPolicy, IntentAction, IntentFileAction, IntentPlan, IntentSelectAction, IntentSource,
    ResolvedOptions,
};
pub use operation::{
    FileOperation, OperationAction, OperationEntry, OperationPlan, OperationPlanningIssue,
    PlannedOperationAction,
};
pub use status::{
    CopyInspection, CopyInspectionState, OperationInspectionOutcome, OperationInspectionReport,
    SymlinkInspection, SymlinkInspectionState, inspect_operation_plan_status,
};

use std::path::{Path, PathBuf};

use crate::boundary::EnvironmentReader;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramOutput {
    pub message: String,
}

const DEFAULT_CONFIG_HOME_SUFFIX: &str = ".config";
const DEFAULT_CONFIG_DIRECTORY_NAME: &str = "gitenv";
const DEFAULT_CONFIG_FILE_NAME: &str = "config.yml";

/// Parse CLI arguments from the provided iterator and run the selected command.
///
/// This is the entry point used by the binary. The binary supplies
/// `std::env::args_os()` as the argument source. Library consumers can supply
/// any iterator of OS strings, which is useful for embedding and testing.
/// To dispatch from an already-parsed [`Cli`] struct, use [`run_cli`] instead.
pub fn run<I, T>(args: I) -> Result<ProgramOutput, ProgramError>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    use clap::Parser;
    let cli = Cli::parse_from(args);
    run_cli(cli)
}

/// Dispatch to the appropriate command using an already-parsed CLI struct.
///
/// Resolves runtime configuration (terminal color, log level) from the CLI
/// struct, then delegates to the selected command's wired workflow.
pub fn run_cli(cli: Cli) -> Result<ProgramOutput, ProgramError> {
    use std::io::IsTerminal;

    let Cli {
        command,
        config_path,
        log_level,
        color,
    } = cli;

    let runtime_config = RuntimeConfig::new(
        color,
        std::io::stdout().is_terminal(),
        std::io::stderr().is_terminal(),
    );

    logging::init(log_level.into(), runtime_config.use_color_for_stderr);

    let boundary = boundary::RealBoundary;
    let home_directory = fs_adapter::resolve_home_directory(&|name| boundary.get_env_var(name))?;
    let config_path = determine_config_path(config_path.as_deref(), &home_directory, &boundary)?;
    let loaded_config = load_config(&config_path)?;

    match command {
        Some(Command::Apply) => run_apply(loaded_config, home_directory, runtime_config),
        Some(Command::Info) | None => run_info(loaded_config, home_directory, runtime_config),
    }
}

/// Run the info workflow using real production adapters.
///
/// Plans and renders the current status of all configured operations.
pub fn run_info(
    loaded_config: LoadedConfig,
    home_directory: PathBuf,
    runtime_config: RuntimeConfig,
) -> Result<ProgramOutput, ProgramError> {
    let context = app::info::InfoCommandContext::new(
        loaded_config,
        home_directory,
        Box::new(derive_intent_plan),
        Box::new(derive_operation_plan),
    );
    app::info::run_info(runtime_config, context)
}

/// Run the apply workflow using real production adapters.
///
/// Plans and applies all configured operations, returning a rendered summary.
pub fn run_apply(
    loaded_config: LoadedConfig,
    home_directory: PathBuf,
    runtime_config: RuntimeConfig,
) -> Result<ProgramOutput, ProgramError> {
    let context = app::apply::ApplyCommandContext::new(
        loaded_config,
        home_directory,
        Box::new(derive_intent_plan),
        Box::new(derive_operation_plan),
        Box::new(apply_operation_plan),
    );
    app::apply::run_apply(runtime_config, context)
}

/// Single production entry point for intent planning.
/// Wires real boundary adapters into an `IntentContext` and delegates to the
/// internal planning function.
pub fn derive_intent_plan(
    loaded_config: &LoadedConfig,
    home_directory: &Path,
) -> Result<IntentPlan, ProgramError> {
    let boundary = boundary::RealBoundary;
    let context = intent::IntentContext {
        home_directory: home_directory.to_path_buf(),
        env_reader: &boundary,
        dir_probe: &boundary,
        config_reader: &boundary,
    };
    intent::derive_intent_plan(loaded_config, &context)
}

/// Single production entry point for operation planning.
/// Wires a real boundary adapter into an `OperationContext` and delegates to
/// the internal planning function.
pub fn derive_operation_plan(
    intent_plan: &IntentPlan,
    home_directory: &Path,
) -> Result<OperationPlan, ProgramError> {
    let boundary = boundary::RealBoundary;
    let context = operation::OperationContext {
        home_directory: home_directory.to_path_buf(),
        dir_reader: &boundary,
        source_reader: &boundary,
        global_selection_excludes: default_global_selection_excludes(std::env::consts::OS),
    };
    operation::derive_operation_plan(intent_plan, &context)
}

/// Single production entry point for apply execution.
/// Wires real boundary adapters into an `ApplyContext` and delegates to the
/// internal execution function.
pub fn apply_operation_plan(
    operation_plan: &OperationPlan,
) -> Result<ApplyOperationReport, ProgramError> {
    let boundary = boundary::RealBoundary;
    let context = actions::ApplyContext {
        target_probe: &boundary,
        symlink_creator: &boundary,
    };
    actions::apply_operation_plan(operation_plan, &context)
}

/// Resolves the effective configuration path from explicit and default sources.
///
/// Priority:
/// 1. Explicit config path from parsed CLI arguments (`-c/--config` or
///    `GITENV_CONFIG` through clap env handling)
/// 2. Default path derived from `XDG_CONFIG_HOME` and home directory
fn determine_config_path(
    explicit_config_path: Option<&Path>,
    home_directory: &Path,
    boundary: &impl EnvironmentReader,
) -> Result<PathBuf, ProgramError> {
    if let Some(path) = explicit_config_path {
        return Ok(path.to_path_buf());
    }

    default_config_path(home_directory, boundary)
}

/// Resolves the default configuration path from environment and home directory.
///
/// Priority:
/// 1. XDG_CONFIG_HOME/gitenv/config.yml if XDG_CONFIG_HOME is set and absolute
/// 2. $HOME/.config/gitenv/config.yml as default
fn default_config_path(
    home_directory: &Path,
    boundary: &impl EnvironmentReader,
) -> Result<PathBuf, ProgramError> {
    let xdg_config_home = boundary.get_env_var("XDG_CONFIG_HOME").map(PathBuf::from);
    Ok(default_config_path_from_env(
        home_directory,
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

fn default_global_selection_excludes(os_name: &str) -> Vec<String> {
    let mut excludes = Vec::new();

    if os_name == "macos" {
        excludes.push(".DS_Store".to_string());
    }

    excludes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boundary::test_doubles::MapEnvReader;
    use clap::Parser;
    use std::path::{Path, PathBuf};

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
    fn use_the_explicit_config_path_before_default_config_resolution() {
        let boundary = MapEnvReader::from_pairs([("XDG_CONFIG_HOME", "/custom/config")]);

        let path = determine_config_path(
            Some(Path::new("/tmp/custom-config.yml")),
            Path::new("/home/alex"),
            &boundary,
        )
        .expect("explicit config path should short-circuit default path resolution");

        assert_eq!(path, PathBuf::from("/tmp/custom-config.yml"));
    }

    #[test]
    fn ignore_gitenv_config_when_resolving_default_config_path() {
        let boundary = MapEnvReader::from_pairs([
            ("GITENV_CONFIG", "/tmp/custom-config.yml"),
            ("XDG_CONFIG_HOME", "/custom/config"),
        ]);

        let path = default_config_path(Path::new("/home/alex"), &boundary)
            .expect("default path should ignore GITENV_CONFIG and use XDG_CONFIG_HOME");

        assert_eq!(path, PathBuf::from("/custom/config/gitenv/config.yml"));
    }

    #[test]
    fn resolve_default_config_path_with_xdg_config_home_via_boundary() {
        let boundary = MapEnvReader::from_pairs([("XDG_CONFIG_HOME", "/custom/config")]);

        let path = default_config_path(Path::new("/home/alex"), &boundary)
            .expect("XDG_CONFIG_HOME should be used when set");

        assert_eq!(path, PathBuf::from("/custom/config/gitenv/config.yml"));
    }

    #[test]
    fn resolve_default_config_path_without_overrides() {
        let boundary = MapEnvReader::empty();

        let path = default_config_path(Path::new("/home/alex"), &boundary)
            .expect("default path should be resolved from home directory");

        assert_eq!(path, PathBuf::from("/home/alex/.config/gitenv/config.yml"));
    }

    #[test]
    fn use_the_explicit_config_path_when_dispatching_the_info_command() {
        let missing_path = PathBuf::from("/path/that/does/not/exist/config.yml");
        let cli = Cli::parse_from([
            "gitenv",
            "--config",
            missing_path
                .to_str()
                .expect("test path should be valid UTF-8"),
        ]);

        let result = run_cli(cli);

        assert!(matches!(
            result,
            Err(ProgramError::ConfigurationReadFailed { path, .. }) if path == missing_path
        ));
    }

    #[test]
    fn use_the_explicit_config_path_when_dispatching_the_apply_command() {
        let missing_path = PathBuf::from("/path/that/does/not/exist/config.yml");
        let cli = Cli::parse_from([
            "gitenv",
            "--config",
            missing_path
                .to_str()
                .expect("test path should be valid UTF-8"),
            "apply",
        ]);

        let result = run_cli(cli);

        assert!(matches!(
            result,
            Err(ProgramError::ConfigurationReadFailed { path, .. }) if path == missing_path
        ));
    }

    #[test]
    fn include_ds_store_in_global_selection_excludes_for_macos() {
        let excludes = default_global_selection_excludes("macos");

        assert_eq!(excludes, vec![".DS_Store".to_string()]);
    }

    #[test]
    fn keep_global_selection_excludes_empty_for_non_macos_os_names() {
        let excludes = default_global_selection_excludes("linux");

        assert_eq!(excludes, Vec::<String>::new());
    }
}
