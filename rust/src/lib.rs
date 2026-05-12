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

pub use actions::{
    ApplyOperationOutcome, ApplyOperationReport, apply_operation_plan_with_injectables,
};
pub use config::{
    ActionMode, Config, ConfigItem, Defaults, FileConfig, Guard, Include, LoadedConfig,
    SelectConfig, Source, SourceRoot, load_config, parse_config,
};
pub use intent::{
    ConflictPolicy, IntentAction, IntentFileAction, IntentPlan, IntentSelectAction, IntentSource,
    ResolvedOptions,
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

use crate::boundary::{DirectoryEntriesReader, EnvironmentReader, SymlinkCreator, TargetProbe};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramOutput {
    pub message: String,
}

const DEFAULT_CONFIG_HOME_SUFFIX: &str = ".config";
const DEFAULT_CONFIG_DIRECTORY_NAME: &str = "gitenv";
const DEFAULT_CONFIG_FILE_NAME: &str = "config.yml";

#[derive(Clone, Copy)]
pub(crate) struct SystemCalls<'a> {
    pub(crate) get_env_var: &'a dyn Fn(&str) -> Option<String>,
}

fn real_system_calls() -> SystemCalls<'static> {
    SystemCalls {
        get_env_var: &|name| boundary::RealBoundary.get_env_var(name),
    }
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
    intent::derive_intent_plan_from_context(loaded_config, &context)
}

/// Derives an operation plan using real filesystem directory reads.
pub fn derive_operation_plan(
    intent_plan: &IntentPlan,
    home_directory: &Path,
) -> Result<OperationPlan, ProgramError> {
    let boundary = boundary::RealBoundary;
    operation::derive_operation_plan_with_injectables(intent_plan, home_directory, &|path| {
        boundary.list_directory_entries(path)
    })
}

/// Applies operations using real filesystem probes and symlink creation.
pub fn apply_operation_plan(
    operation_plan: &OperationPlan,
) -> Result<ApplyOperationReport, ProgramError> {
    let boundary = boundary::RealBoundary;
    apply_operation_plan_with_injectables(
        operation_plan,
        &|path| boundary.target_exists(path),
        &|source, target| boundary.create_symlink(source, target),
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
        || app::info::run_info(info_command_context_with_system(system)?, runtime_config),
        || app::apply::run_apply(apply_command_context_with_system(system)?, runtime_config),
    )
}

fn info_command_context_with_system(
    system: SystemCalls<'_>,
) -> Result<app::info::InfoCommandContext, ProgramError> {
    Ok(app::info::InfoCommandContext::new(
        load_default_config_with_system(system)?,
        fs_adapter::resolve_home_directory(system.get_env_var)?,
        Box::new(derive_intent_plan),
        Box::new(derive_operation_plan),
    ))
}

fn apply_command_context_with_system(
    system: SystemCalls<'_>,
) -> Result<app::apply::ApplyCommandContext, ProgramError> {
    Ok(app::apply::ApplyCommandContext::new(
        load_default_config_with_system(system)?,
        fs_adapter::resolve_home_directory(system.get_env_var)?,
        Box::new(derive_intent_plan),
        Box::new(derive_operation_plan),
        Box::new(apply_operation_plan),
    ))
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
        ProgramError, SystemCalls, default_config_path_from_env, default_config_path_with_system,
        load_default_config_with_system,
    };
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

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
