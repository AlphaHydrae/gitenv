mod actions;
mod cli;
mod config;
mod fs_adapter;
mod intent;
mod operation;
mod status;

pub use cli::{Cli, Command, dispatch};

pub use actions::{
    ApplyOperationOutcome, ApplyOperationReport, apply_operation_plan,
    apply_operation_plan_with_injectables,
};
pub use config::{
    ActionMode, Config, ConfigItem, Defaults, FileConfig, Guard, Include, SelectConfig, Source,
    SourceRoot, load_config, parse_config,
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
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

const ANSI_RESET: &str = "\x1b[0m";
const ANSI_GREEN: &str = "\x1b[32m";
const ANSI_YELLOW: &str = "\x1b[33m";
const ANSI_RED: &str = "\x1b[31m";

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
    UnsupportedConfiguration,
}

const DEFAULT_CONFIG_HOME_SUFFIX: &str = ".config";
const DEFAULT_CONFIG_DIRECTORY_NAME: &str = "gitenv";
const DEFAULT_CONFIG_FILE_NAME: &str = "config.yml";

#[derive(Clone, Copy)]
struct SystemCalls<'a> {
    get_env_var_os: &'a dyn Fn(&str) -> Option<OsString>,
    stdout_is_terminal: &'a dyn Fn() -> bool,
}

fn std_env_var_os(name: &str) -> Option<OsString> {
    std::env::var_os(name)
}

fn std_stdout_is_terminal() -> bool {
    std::io::stdout().is_terminal()
}

fn real_system_calls() -> SystemCalls<'static> {
    SystemCalls {
        get_env_var_os: &std_env_var_os,
        stdout_is_terminal: &std_stdout_is_terminal,
    }
}

/// Parse CLI arguments and dispatch to the appropriate library function.
///
/// This is the main entry point for the binary.
pub fn run_cli() -> Result<ProgramOutput, ProgramError> {
    run_cli_with_args(std::env::args_os())
}

/// Parse injected CLI arguments and dispatch to the appropriate library function.
///
/// This keeps command-line parsing testable without mutating process args.
pub fn run_cli_with_args<I, T>(args: I) -> Result<ProgramOutput, ProgramError>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    use clap::Parser;
    cli::dispatch(Cli::parse_from(args))
}

fn run_with_config_path_with_system(
    config_path: &Path,
    system: SystemCalls<'_>,
) -> Result<ProgramOutput, ProgramError> {
    let config = load_config(config_path)?;
    let intent_plan = derive_intent_plan(&config)?;
    let get_home_directory = || fs_adapter::resolve_home_directory(system.get_env_var_os);
    let operation_plan = operation::derive_operation_plan_with_injectables(
        &intent_plan,
        &get_home_directory,
        &fs_adapter::list_directory_entries,
    )?;

    Ok(ProgramOutput {
        message: render_default_inspection_output_with_system(&operation_plan, system)?,
    })
}

/// Run the info command (status inspection for all operations).
pub fn run_info() -> Result<ProgramOutput, ProgramError> {
    let system = real_system_calls();
    run_info_from_env_with_system(|| default_config_path_with_system(system), system)
}

fn run_info_from_env_with_system(
    get_config_path: impl FnOnce() -> Result<PathBuf, ProgramError>,
    system: SystemCalls<'_>,
) -> Result<ProgramOutput, ProgramError> {
    let config_path = get_config_path()?;
    run_with_config_path_with_system(&config_path, system)
}

/// Run the apply command (execute all operations).
pub fn run_apply() -> Result<ProgramOutput, ProgramError> {
    let system = real_system_calls();
    run_apply_from_env_with_system(|| default_config_path_with_system(system), system)
}

fn run_apply_from_env_with_system(
    get_config_path: impl FnOnce() -> Result<PathBuf, ProgramError>,
    system: SystemCalls<'_>,
) -> Result<ProgramOutput, ProgramError> {
    let config_path = get_config_path()?;
    run_apply_with_config_path_with_system(&config_path, system)
}

fn run_apply_with_config_path_with_system(
    config_path: &Path,
    system: SystemCalls<'_>,
) -> Result<ProgramOutput, ProgramError> {
    let config = load_config(config_path)?;
    let intent_plan = derive_intent_plan(&config)?;
    let get_home_directory = || fs_adapter::resolve_home_directory(system.get_env_var_os);
    let operation_plan = operation::derive_operation_plan_with_injectables(
        &intent_plan,
        &get_home_directory,
        &fs_adapter::list_directory_entries,
    )?;

    let apply_report = apply_operation_plan(&operation_plan)?;
    let output_message = render_apply_output_with_system(&apply_report, system)?;

    Ok(ProgramOutput {
        message: output_message,
    })
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

fn render_default_inspection_output_with_system(
    operation_plan: &OperationPlan,
    system: SystemCalls<'_>,
) -> Result<String, ProgramError> {
    let use_color = should_use_color(system);
    let inspection_report = inspect_operation_plan_status(operation_plan)?;
    let lines = inspection_report
        .outcomes
        .iter()
        .map(|outcome| render_operation_inspection_line_with_color(outcome, use_color))
        .collect::<Vec<_>>();

    if lines.is_empty() {
        Ok("No operations to inspect.".to_string())
    } else {
        Ok(lines.join("\n"))
    }
}

fn render_operation_inspection_line_with_color(
    outcome: &OperationInspectionOutcome,
    use_color: bool,
) -> String {
    match outcome {
        OperationInspectionOutcome::Symlink(inspection) => {
            render_symlink_inspection_line_with_color(inspection, use_color)
        }
        OperationInspectionOutcome::Copy(inspection) => {
            render_copy_inspection_line_with_color(inspection, use_color)
        }
    }
}

fn render_symlink_inspection_line_with_color(
    inspection: &SymlinkInspection,
    use_color: bool,
) -> String {
    let state = match &inspection.state {
        SymlinkInspectionState::Ok => colorize("ok", ANSI_GREEN, use_color),
        SymlinkInspectionState::Missing => colorize("not yet set up", ANSI_YELLOW, use_color),
        SymlinkInspectionState::NotASymlink => colorize("not a symlink", ANSI_RED, use_color),
        SymlinkInspectionState::PointsElsewhere { current_target } => colorize(
            &format!("points to {}", current_target.display()),
            ANSI_RED,
            use_color,
        ),
    };

    format!(
        "{} -> {}   {}",
        inspection.target.display(),
        inspection.source.display(),
        state
    )
}

fn render_copy_inspection_line_with_color(inspection: &CopyInspection, use_color: bool) -> String {
    let state = match inspection.state {
        CopyInspectionState::Ok => colorize("ok", ANSI_GREEN, use_color),
        CopyInspectionState::Missing => colorize("not yet set up", ANSI_YELLOW, use_color),
        CopyInspectionState::NotAFile => colorize("not a file", ANSI_RED, use_color),
        CopyInspectionState::Differs => colorize("differs from source", ANSI_RED, use_color),
    };

    format!(
        "{} <- {}   {}",
        inspection.target.display(),
        inspection.source.display(),
        state
    )
}

fn render_apply_output_with_system(
    apply_report: &ApplyOperationReport,
    system: SystemCalls<'_>,
) -> Result<String, ProgramError> {
    let use_color = should_use_color(system);
    let lines = apply_report
        .outcomes
        .iter()
        .map(|outcome| render_apply_outcome_line_with_color(outcome, use_color))
        .collect::<Vec<_>>();

    if lines.is_empty() {
        Ok("No operations to apply.".to_string())
    } else {
        Ok(lines.join("\n"))
    }
}

fn render_apply_outcome_line_with_color(
    outcome: &ApplyOperationOutcome,
    use_color: bool,
) -> String {
    match outcome {
        ApplyOperationOutcome::Applied(action) => match action {
            OperationAction::Symlink(op) => {
                format!(
                    "{} {} -> {}",
                    colorize("created symlink", ANSI_GREEN, use_color),
                    op.target.display(),
                    op.source.display()
                )
            }
            OperationAction::Copy(op) => {
                format!(
                    "{} {} to {}",
                    colorize("copied", ANSI_GREEN, use_color),
                    op.source.display(),
                    op.target.display()
                )
            }
        },
        ApplyOperationOutcome::SkippedExistingTarget(action) => match action {
            OperationAction::Symlink(op) => {
                format!(
                    "{} {} (already exists)",
                    colorize("skipped symlink", ANSI_YELLOW, use_color),
                    op.target.display()
                )
            }
            OperationAction::Copy(op) => {
                format!(
                    "{} {} (already exists)",
                    colorize("skipped copy", ANSI_YELLOW, use_color),
                    op.target.display()
                )
            }
        },
        ApplyOperationOutcome::UnsupportedOperation(action) => match action {
            OperationAction::Symlink(op) => {
                format!(
                    "{} {} -> {}",
                    colorize("unsupported: symlink", ANSI_RED, use_color),
                    op.target.display(),
                    op.source.display()
                )
            }
            OperationAction::Copy(op) => {
                format!(
                    "{} {} to {}",
                    colorize("unsupported: copy", ANSI_RED, use_color),
                    op.source.display(),
                    op.target.display()
                )
            }
        },
    }
}

fn should_use_color(system: SystemCalls<'_>) -> bool {
    (system.stdout_is_terminal)() && (system.get_env_var_os)("NO_COLOR").is_none()
}

fn colorize(text: &str, color: &str, use_color: bool) -> String {
    if use_color {
        format!("{color}{text}{ANSI_RESET}")
    } else {
        text.to_string()
    }
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
            ProgramError::UnsupportedConfiguration => write!(
                f,
                "default inspection currently supports symlink operations only"
            ),
        }
    }
}

impl std::error::Error for ProgramError {}

#[cfg(test)]
mod tests {
    use super::{
        ApplyOperationOutcome, ConflictPolicy, CopyInspection, CopyInspectionState, FileOperation,
        OperationAction, OperationInspectionOutcome, OperationPlan, ProgramError, ProgramOutput,
        SymlinkInspection, SymlinkInspectionState, SystemCalls, default_config_path_from_env,
        default_config_path_from_inputs, render_apply_outcome_line_with_color,
        render_apply_output_with_system, render_copy_inspection_line_with_color,
        render_default_inspection_output_with_system, render_operation_inspection_line_with_color,
        render_symlink_inspection_line_with_color, run_apply_from_env_with_system,
        run_apply_with_config_path_with_system, run_info_from_env_with_system,
        run_with_config_path_with_system,
    };
    use std::collections::BTreeMap;
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn render_a_missing_symlink_status_line() {
        let line = render_symlink_inspection_line_with_color(
            &SymlinkInspection {
                source: PathBuf::from("/repo/.gitconfig"),
                target: PathBuf::from("/home/.gitconfig"),
                state: SymlinkInspectionState::Missing,
            },
            false,
        );

        assert_eq!(
            line,
            "/home/.gitconfig -> /repo/.gitconfig   not yet set up"
        );
    }

    #[test]
    fn render_all_non_missing_symlink_status_lines() {
        let ok_line = render_symlink_inspection_line_with_color(
            &SymlinkInspection {
                source: PathBuf::from("/repo/.zshrc"),
                target: PathBuf::from("/home/.zshrc"),
                state: SymlinkInspectionState::Ok,
            },
            false,
        );
        let not_a_symlink_line = render_symlink_inspection_line_with_color(
            &SymlinkInspection {
                source: PathBuf::from("/repo/.zprofile"),
                target: PathBuf::from("/home/.zprofile"),
                state: SymlinkInspectionState::NotASymlink,
            },
            false,
        );
        let points_elsewhere_line = render_symlink_inspection_line_with_color(
            &SymlinkInspection {
                source: PathBuf::from("/repo/.gitconfig"),
                target: PathBuf::from("/home/.gitconfig"),
                state: SymlinkInspectionState::PointsElsewhere {
                    current_target: PathBuf::from("/old/.gitconfig"),
                },
            },
            false,
        );

        assert_eq!(ok_line, "/home/.zshrc -> /repo/.zshrc   ok");
        assert_eq!(
            not_a_symlink_line,
            "/home/.zprofile -> /repo/.zprofile   not a symlink"
        );
        assert_eq!(
            points_elsewhere_line,
            "/home/.gitconfig -> /repo/.gitconfig   points to /old/.gitconfig"
        );
    }

    #[test]
    fn render_all_copy_status_lines() {
        let ok_line = render_copy_inspection_line_with_color(
            &CopyInspection {
                source: PathBuf::from("/repo/.zshrc"),
                target: PathBuf::from("/home/.zshrc"),
                state: CopyInspectionState::Ok,
            },
            false,
        );
        let missing_line = render_copy_inspection_line_with_color(
            &CopyInspection {
                source: PathBuf::from("/repo/.gitconfig"),
                target: PathBuf::from("/home/.gitconfig"),
                state: CopyInspectionState::Missing,
            },
            false,
        );
        let not_a_file_line = render_copy_inspection_line_with_color(
            &CopyInspection {
                source: PathBuf::from("/repo/.bashrc"),
                target: PathBuf::from("/home/.bashrc"),
                state: CopyInspectionState::NotAFile,
            },
            false,
        );
        let differs_line = render_copy_inspection_line_with_color(
            &CopyInspection {
                source: PathBuf::from("/repo/.vimrc"),
                target: PathBuf::from("/home/.vimrc"),
                state: CopyInspectionState::Differs,
            },
            false,
        );

        assert_eq!(ok_line, "/home/.zshrc <- /repo/.zshrc   ok");
        assert_eq!(
            missing_line,
            "/home/.gitconfig <- /repo/.gitconfig   not yet set up"
        );
        assert_eq!(
            not_a_file_line,
            "/home/.bashrc <- /repo/.bashrc   not a file"
        );
        assert_eq!(
            differs_line,
            "/home/.vimrc <- /repo/.vimrc   differs from source"
        );
    }

    #[test]
    fn render_operation_status_line_for_each_operation_kind() {
        let symlink_line = render_operation_inspection_line_with_color(
            &OperationInspectionOutcome::Symlink(SymlinkInspection {
                source: PathBuf::from("/repo/.zshrc"),
                target: PathBuf::from("/home/.zshrc"),
                state: SymlinkInspectionState::Missing,
            }),
            false,
        );
        let copy_line = render_operation_inspection_line_with_color(
            &OperationInspectionOutcome::Copy(CopyInspection {
                source: PathBuf::from("/repo/.gitconfig"),
                target: PathBuf::from("/home/.gitconfig"),
                state: CopyInspectionState::Missing,
            }),
            false,
        );

        assert_eq!(
            symlink_line,
            "/home/.zshrc -> /repo/.zshrc   not yet set up"
        );
        assert_eq!(
            copy_line,
            "/home/.gitconfig <- /repo/.gitconfig   not yet set up"
        );
    }

    #[test]
    fn render_no_operation_message_when_nothing_is_planned() {
        let get_env_var_os = |_: &str| None::<OsString>;
        let stdout_is_terminal = || false;
        let output = render_default_inspection_output_with_system(
            &OperationPlan { actions: vec![] },
            SystemCalls {
                get_env_var_os: &get_env_var_os,
                stdout_is_terminal: &stdout_is_terminal,
            },
        )
        .expect("empty operation plans should render a stable status message");

        assert_eq!(
            output,
            ProgramOutput {
                message: "No operations to inspect.".to_string(),
            }
            .message
        );
    }

    #[test]
    fn inspect_copy_operations_in_the_default_inspection_flow() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source = temp.path().join("source.txt");
        let target = temp.path().join("target.txt");
        fs::write(&source, "source\n").expect("source file should be written");

        let operation_plan = OperationPlan {
            actions: vec![OperationAction::Copy(FileOperation {
                source: source.clone(),
                target: target.clone(),
                mkdir: true,
                conflict_policy: ConflictPolicy::Skip,
            })],
        };

        let get_env_var_os = |_: &str| None::<OsString>;
        let stdout_is_terminal = || false;
        let output = render_default_inspection_output_with_system(
            &operation_plan,
            SystemCalls {
                get_env_var_os: &get_env_var_os,
                stdout_is_terminal: &stdout_is_terminal,
            },
        )
        .expect("copy operations should render deterministic inspection output");

        assert_eq!(
            output,
            format!(
                "{} <- {}   not yet set up",
                target.display(),
                source.display(),
            )
        );
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
        let stdout_is_terminal = || false;
        let output = run_with_config_path_with_system(
            &config_path,
            SystemCalls {
                get_env_var_os: &get_env_var_os,
                stdout_is_terminal: &stdout_is_terminal,
            },
        )
        .expect("explicit config path should produce inspection output");

        let expected = format!(
            "{} -> {}   not yet set up",
            home.path().join(".gitconfig").display(),
            repository.path().join(".").join(".gitconfig").display()
        );
        assert_eq!(output.message, expected);
    }

    #[test]
    fn show_setup_guidance_when_file_is_missing() {
        let missing_path = PathBuf::from("/tmp/custom-gitenv.yml");

        let get_env_var_os = |_: &str| None::<OsString>;
        let stdout_is_terminal = || false;
        let error = run_with_config_path_with_system(
            &missing_path,
            SystemCalls {
                get_env_var_os: &get_env_var_os,
                stdout_is_terminal: &stdout_is_terminal,
            },
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
        let stdout_is_terminal = || false;
        let output = run_apply_with_config_path_with_system(
            &config_path,
            SystemCalls {
                get_env_var_os: &get_env_var_os,
                stdout_is_terminal: &stdout_is_terminal,
            },
        )
        .expect("explicit config path should produce apply output");

        let expected = format!(
            "created symlink {} -> {}",
            home.path().join(".gitconfig").display(),
            repository.path().join(".").join(".gitconfig").display()
        );
        assert_eq!(output.message, expected);
    }

    #[test]
    fn show_home_missing_errors_for_explicit_config_info_and_apply() {
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

        let get_env_var_os = |_: &str| None::<OsString>;
        let stdout_is_terminal = || false;

        let info_error = run_with_config_path_with_system(
            &config_path,
            SystemCalls {
                get_env_var_os: &get_env_var_os,
                stdout_is_terminal: &stdout_is_terminal,
            },
        )
        .expect_err("missing HOME should fail explicit-config info planning");
        assert_eq!(info_error, ProgramError::HomeDirectoryUnavailable);

        let apply_error = run_apply_with_config_path_with_system(
            &config_path,
            SystemCalls {
                get_env_var_os: &get_env_var_os,
                stdout_is_terminal: &stdout_is_terminal,
            },
        )
        .expect_err("missing HOME should fail explicit-config apply planning");
        assert_eq!(apply_error, ProgramError::HomeDirectoryUnavailable);
    }

    #[test]
    fn show_default_inspection_output() {
        // Verifies the env-injection path of run_info() by supplying a known
        // config path directly, without touching process environment variables.
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
        let stdout_is_terminal = || false;
        let output = run_info_from_env_with_system(
            || Ok(config_path),
            SystemCalls {
                get_env_var_os: &get_env_var_os,
                stdout_is_terminal: &stdout_is_terminal,
            },
        )
        .expect("run_info_from_env should succeed with injected config path");

        let expected = format!(
            "{} -> {}   not yet set up",
            home.path().join(".profile").display(),
            repository.path().join(".").join(".profile").display()
        );
        assert_eq!(output.message, expected);
    }

    #[test]
    fn show_apply_output() {
        // Verifies the env-injection path of run_apply() by supplying a known
        // config path directly, without touching process environment variables.
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
        let stdout_is_terminal = || false;
        let output = run_apply_from_env_with_system(
            || Ok(config_path),
            SystemCalls {
                get_env_var_os: &get_env_var_os,
                stdout_is_terminal: &stdout_is_terminal,
            },
        )
        .expect("run_apply_from_env should succeed with injected config path");

        let expected = format!(
            "created symlink {} -> {}",
            home.path().join(".profile").display(),
            repository.path().join(".").join(".profile").display()
        );
        assert_eq!(output.message, expected);
    }

    #[test]
    fn show_no_operations_to_apply_message_when_outcomes_are_empty() {
        let report = super::ApplyOperationReport { outcomes: vec![] };

        let get_env_var_os = |_: &str| None::<OsString>;
        let stdout_is_terminal = || false;
        let output = render_apply_output_with_system(
            &report,
            SystemCalls {
                get_env_var_os: &get_env_var_os,
                stdout_is_terminal: &stdout_is_terminal,
            },
        )
        .expect("empty apply report should render a stable message");

        assert_eq!(output, "No operations to apply.");
    }

    #[test]
    fn show_applied_operation_lines_for_symlink_and_copy_actions() {
        let symlink_op = FileOperation {
            source: PathBuf::from("/repo/.zshrc"),
            target: PathBuf::from("/home/.zshrc"),
            mkdir: false,
            conflict_policy: ConflictPolicy::Skip,
        };
        let copy_op = FileOperation {
            source: PathBuf::from("/repo/config.txt"),
            target: PathBuf::from("/home/config.txt"),
            mkdir: false,
            conflict_policy: ConflictPolicy::Skip,
        };

        let symlink_line = render_apply_outcome_line_with_color(
            &ApplyOperationOutcome::Applied(OperationAction::Symlink(symlink_op.clone())),
            false,
        );
        let copy_line = render_apply_outcome_line_with_color(
            &ApplyOperationOutcome::Applied(OperationAction::Copy(copy_op.clone())),
            false,
        );

        assert_eq!(symlink_line, "created symlink /home/.zshrc -> /repo/.zshrc");
        assert_eq!(copy_line, "copied /repo/config.txt to /home/config.txt");
    }

    #[test]
    fn show_skipped_operation_lines_for_symlink_and_copy_actions() {
        let symlink_op = FileOperation {
            source: PathBuf::from("/repo/.zshrc"),
            target: PathBuf::from("/home/.zshrc"),
            mkdir: false,
            conflict_policy: ConflictPolicy::Skip,
        };
        let copy_op = FileOperation {
            source: PathBuf::from("/repo/config.txt"),
            target: PathBuf::from("/home/config.txt"),
            mkdir: false,
            conflict_policy: ConflictPolicy::Skip,
        };

        let symlink_line = render_apply_outcome_line_with_color(
            &ApplyOperationOutcome::SkippedExistingTarget(OperationAction::Symlink(
                symlink_op.clone(),
            )),
            false,
        );
        let copy_line = render_apply_outcome_line_with_color(
            &ApplyOperationOutcome::SkippedExistingTarget(OperationAction::Copy(copy_op.clone())),
            false,
        );

        assert_eq!(
            symlink_line,
            "skipped symlink /home/.zshrc (already exists)"
        );
        assert_eq!(copy_line, "skipped copy /home/config.txt (already exists)");
    }

    #[test]
    fn show_unsupported_operation_lines_for_symlink_and_copy_actions() {
        // UnsupportedOperation is a reserved variant in the public API for
        // future use; it is never generated by the current executor but the
        // renderer must handle it for exhaustive match completeness.
        let symlink_op = FileOperation {
            source: PathBuf::from("/repo/.zshrc"),
            target: PathBuf::from("/home/.zshrc"),
            mkdir: false,
            conflict_policy: ConflictPolicy::Skip,
        };
        let copy_op = FileOperation {
            source: PathBuf::from("/repo/config.txt"),
            target: PathBuf::from("/home/config.txt"),
            mkdir: false,
            conflict_policy: ConflictPolicy::Skip,
        };

        let symlink_line = render_apply_outcome_line_with_color(
            &ApplyOperationOutcome::UnsupportedOperation(OperationAction::Symlink(
                symlink_op.clone(),
            )),
            false,
        );
        let copy_line = render_apply_outcome_line_with_color(
            &ApplyOperationOutcome::UnsupportedOperation(OperationAction::Copy(copy_op.clone())),
            false,
        );

        assert_eq!(
            symlink_line,
            "unsupported: symlink /home/.zshrc -> /repo/.zshrc"
        );
        assert_eq!(
            copy_line,
            "unsupported: copy /repo/config.txt to /home/config.txt"
        );
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
        assert_eq!(
            ProgramError::UnsupportedConfiguration.to_string(),
            "default inspection currently supports symlink operations only"
        );
    }

    #[test]
    fn render_colorized_primary_status_states() {
        let symlink_ok = render_symlink_inspection_line_with_color(
            &SymlinkInspection {
                source: PathBuf::from("/repo/.zshrc"),
                target: PathBuf::from("/home/.zshrc"),
                state: SymlinkInspectionState::Ok,
            },
            true,
        );
        let symlink_missing = render_symlink_inspection_line_with_color(
            &SymlinkInspection {
                source: PathBuf::from("/repo/.gitconfig"),
                target: PathBuf::from("/home/.gitconfig"),
                state: SymlinkInspectionState::Missing,
            },
            true,
        );
        let symlink_mismatch = render_symlink_inspection_line_with_color(
            &SymlinkInspection {
                source: PathBuf::from("/repo/.profile"),
                target: PathBuf::from("/home/.profile"),
                state: SymlinkInspectionState::NotASymlink,
            },
            true,
        );

        assert!(symlink_ok.contains("\x1b[32mok\x1b[0m"));
        assert!(symlink_missing.contains("\x1b[33mnot yet set up\x1b[0m"));
        assert!(symlink_mismatch.contains("\x1b[31mnot a symlink\x1b[0m"));
    }

    #[test]
    fn render_colorized_apply_outcome_prefixes() {
        let operation = FileOperation {
            source: PathBuf::from("/repo/.gitconfig"),
            target: PathBuf::from("/home/.gitconfig"),
            mkdir: false,
            conflict_policy: ConflictPolicy::Skip,
        };

        let applied = render_apply_outcome_line_with_color(
            &ApplyOperationOutcome::Applied(OperationAction::Symlink(operation.clone())),
            true,
        );
        let skipped = render_apply_outcome_line_with_color(
            &ApplyOperationOutcome::SkippedExistingTarget(OperationAction::Symlink(
                operation.clone(),
            )),
            true,
        );
        let unsupported = render_apply_outcome_line_with_color(
            &ApplyOperationOutcome::UnsupportedOperation(OperationAction::Symlink(operation)),
            true,
        );

        assert!(applied.starts_with("\x1b[32mcreated symlink\x1b[0m"));
        assert!(skipped.starts_with("\x1b[33mskipped symlink\x1b[0m"));
        assert!(unsupported.starts_with("\x1b[31munsupported: symlink\x1b[0m"));
    }
}
