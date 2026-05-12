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
pub use errors::ProgramError;
pub use logging::init as init_logging;

pub use actions::{ApplyOperationOutcome, ApplyOperationReport};
pub use config::{
    ActionMode, Config, ConfigItem, Defaults, FileConfig, Guard, Include, LoadedConfig,
    SelectConfig, Source, SourceRoot, load_config, parse_config,
};
pub use intent::{
    ConflictPolicy, IntentAction, IntentFileAction, IntentPlan, IntentSelectAction, IntentSource,
    ResolvedOptions,
};
pub use operation::{FileOperation, OperationAction, OperationPlan};
pub use status::{
    CopyInspection, CopyInspectionState, OperationInspectionOutcome, OperationInspectionReport,
    SymlinkInspection, SymlinkInspectionState, inspect_operation_plan_status,
    inspect_symlink_operation_status,
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

    // Composition root: resolve runtime facts and wire stage entrypoints.
    let boundary = boundary::RealBoundary;
    let home_directory = fs_adapter::resolve_home_directory(&|name| boundary.get_env_var(name))?;
    let config_path = default_config_path(&home_directory, &boundary)?;
    let loaded_config = load_config(&config_path)?;

    cli::dispatch_with(
        cli,
        || {
            let context = app::info::InfoCommandContext::new(
                loaded_config.clone(),
                home_directory.clone(),
                Box::new(derive_intent_plan),
                Box::new(derive_operation_plan),
            );
            app::info::run_info(context, runtime_config)
        },
        || {
            let context = app::apply::ApplyCommandContext::new(
                loaded_config.clone(),
                home_directory.clone(),
                Box::new(derive_intent_plan),
                Box::new(derive_operation_plan),
                Box::new(apply_operation_plan),
            );
            app::apply::run_apply(context, runtime_config)
        },
    )
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

/// Resolves the default configuration path from environment and home directory.
///
/// Priority:
/// 1. GITENV_CONFIG environment variable if set
/// 2. XDG_CONFIG_HOME/gitenv/config.yml if XDG_CONFIG_HOME is set and absolute
/// 3. $HOME/.config/gitenv/config.yml as default
fn default_config_path(
    home_directory: &Path,
    boundary: &impl EnvironmentReader,
) -> Result<PathBuf, ProgramError> {
    if let Some(custom_path) = boundary.get_env_var("GITENV_CONFIG") {
        return Ok(PathBuf::from(custom_path));
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boundary::test_doubles::MapEnvReader;
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
    fn use_gitenv_config_override_in_default_path_resolution() {
        let boundary = MapEnvReader::from_pairs([("GITENV_CONFIG", "/tmp/custom-config.yml")]);

        let path = default_config_path(Path::new("/home/alex"), &boundary)
            .expect("GITENV_CONFIG should short-circuit default path resolution");

        assert_eq!(path, PathBuf::from("/tmp/custom-config.yml"));
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
}
