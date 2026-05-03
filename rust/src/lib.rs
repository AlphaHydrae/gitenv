mod actions;
mod config;
mod intent;
mod operation;
mod status;

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
pub use status::{SymlinkInspection, SymlinkInspectionState, inspect_symlink_operation_status};

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

pub fn run() -> Result<ProgramOutput, ProgramError> {
    let config_path = default_config_path()?;
    run_with_config_path(&config_path)
}

fn run_with_config_path(config_path: &Path) -> Result<ProgramOutput, ProgramError> {
    let config = load_config(config_path)?;
    let intent_plan = derive_intent_plan(&config)?;
    let operation_plan = derive_operation_plan(&intent_plan)?;

    Ok(ProgramOutput {
        message: render_default_inspection_output(&operation_plan)?,
    })
}

fn default_config_path() -> Result<PathBuf, ProgramError> {
    let gitenv_config = std::env::var_os("GITENV_CONFIG").map(PathBuf::from);
    let home_directory = std::env::var_os("HOME").map(PathBuf::from);
    let xdg_config_home = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);

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

fn render_default_inspection_output(
    operation_plan: &OperationPlan,
) -> Result<String, ProgramError> {
    let mut lines = Vec::new();

    for action in &operation_plan.actions {
        match action {
            OperationAction::Symlink(operation) => {
                let inspection = inspect_symlink_operation_status(operation)?;
                lines.push(render_symlink_inspection_line(&inspection));
            }
            OperationAction::Copy(_) => return Err(ProgramError::UnsupportedConfiguration),
        }
    }

    if lines.is_empty() {
        Ok("No operations to inspect.".to_string())
    } else {
        Ok(lines.join("\n"))
    }
}

fn render_symlink_inspection_line(inspection: &SymlinkInspection) -> String {
    let state = match &inspection.state {
        SymlinkInspectionState::Ok => "ok".to_string(),
        SymlinkInspectionState::Missing => "not yet set up".to_string(),
        SymlinkInspectionState::NotASymlink => "not a symlink".to_string(),
        SymlinkInspectionState::PointsElsewhere { current_target } => {
            format!("points to {}", current_target.display())
        }
    };

    format!(
        "{} -> {}   {}",
        inspection.target.display(),
        inspection.source.display(),
        state
    )
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
        ConflictPolicy, FileOperation, OperationAction, OperationPlan, ProgramError, ProgramOutput,
        SymlinkInspection, SymlinkInspectionState, default_config_path_from_env,
        default_config_path_from_inputs, render_default_inspection_output,
        render_symlink_inspection_line, run_with_config_path,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn render_a_missing_symlink_status_line() {
        let line = render_symlink_inspection_line(&SymlinkInspection {
            source: PathBuf::from("/repo/.gitconfig"),
            target: PathBuf::from("/home/.gitconfig"),
            state: SymlinkInspectionState::Missing,
        });

        assert_eq!(
            line,
            "/home/.gitconfig -> /repo/.gitconfig   not yet set up"
        );
    }

    #[test]
    fn render_all_non_missing_symlink_status_lines() {
        let ok_line = render_symlink_inspection_line(&SymlinkInspection {
            source: PathBuf::from("/repo/.zshrc"),
            target: PathBuf::from("/home/.zshrc"),
            state: SymlinkInspectionState::Ok,
        });
        let not_a_symlink_line = render_symlink_inspection_line(&SymlinkInspection {
            source: PathBuf::from("/repo/.zprofile"),
            target: PathBuf::from("/home/.zprofile"),
            state: SymlinkInspectionState::NotASymlink,
        });
        let points_elsewhere_line = render_symlink_inspection_line(&SymlinkInspection {
            source: PathBuf::from("/repo/.gitconfig"),
            target: PathBuf::from("/home/.gitconfig"),
            state: SymlinkInspectionState::PointsElsewhere {
                current_target: PathBuf::from("/old/.gitconfig"),
            },
        });

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
    fn render_no_operation_message_when_nothing_is_planned() {
        let output = render_default_inspection_output(&OperationPlan { actions: vec![] })
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
    fn reject_copy_operations_in_the_default_inspection_flow() {
        let operation_plan = OperationPlan {
            actions: vec![OperationAction::Copy(FileOperation {
                source: PathBuf::from("/repo/.gitconfig"),
                target: PathBuf::from("/home/.gitconfig"),
                mkdir: true,
                conflict_policy: ConflictPolicy::Skip,
            })],
        };

        let error = render_default_inspection_output(&operation_plan)
            .expect_err("copy operations are not yet supported in default inspection");

        assert_eq!(error, ProgramError::UnsupportedConfiguration);
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
    fn show_default_inspection_output_for_a_missing_symlink_with_explicit_config_path() {
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

        let output = run_with_config_path(&config_path)
            .expect("explicit config path should produce inspection output");

        let expected = format!(
            "{} -> {}   not yet set up",
            home.path().join(".gitconfig").display(),
            repository.path().join(".").join(".gitconfig").display()
        );
        assert_eq!(output.message, expected);
    }

    #[test]
    fn show_setup_guidance_with_the_configured_config_path_when_file_is_missing() {
        let missing_path = PathBuf::from("/tmp/custom-gitenv.yml");

        let error = run_with_config_path(&missing_path)
            .expect_err("missing explicit config file should return a read error");

        let rendered = error.to_string();
        assert!(rendered.contains("cannot read config at /tmp/custom-gitenv.yml ("));
        assert!(rendered.contains("Config file locations"));
        assert!(rendered.contains("$GITENV_CONFIG (if set)"));
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
}
