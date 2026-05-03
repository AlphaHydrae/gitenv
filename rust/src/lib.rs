mod config;
mod intent;
mod operation;
mod status;

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
    /// The current home directory is required to resolve home-relative paths.
    HomeDirectoryUnavailable,
    UnsupportedConfiguration,
}

const DEFAULT_CONFIG_HOME_SUFFIX: &str = ".config";
const DEFAULT_CONFIG_DIRECTORY_NAME: &str = "gitenv";
const DEFAULT_CONFIG_FILE_NAME: &str = "config.yml";

pub fn run() -> Result<ProgramOutput, ProgramError> {
    let config_path = default_config_path()?;
    let config = load_config(&config_path)?;
    let intent_plan = derive_intent_plan(&config)?;
    let operation_plan = derive_operation_plan(&intent_plan)?;

    Ok(ProgramOutput {
        message: render_default_inspection_output(&operation_plan)?,
    })
}

fn default_config_path() -> Result<PathBuf, ProgramError> {
    let home_directory = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or(ProgramError::HomeDirectoryUnavailable)?;
    let xdg_config_home = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);

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
                write!(f, "invalid configuration: {message}")
            }
            ProgramError::ReadConfiguration { path, message } => {
                write!(f, "failed to read config at {}: {message}", path.display())
            }
            ProgramError::MissingEnvironment { vars } => {
                write!(
                    f,
                    "missing required environment variables: {}",
                    vars.join(", ")
                )
            }
            ProgramError::IncludeNotFound { paths } => write!(
                f,
                "required include files not found: {}",
                paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            ProgramError::IncludeCycle { cycle } => write!(
                f,
                "include cycle detected: {}",
                cycle
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(" -> ")
            ),
            ProgramError::ReadSourceDirectory { path, message } => {
                write!(
                    f,
                    "failed to read source directory {}: {message}",
                    path.display()
                )
            }
            ProgramError::InspectPathMetadata { path, message } => {
                write!(
                    f,
                    "failed to inspect path metadata for {}: {message}",
                    path.display()
                )
            }
            ProgramError::ReadSymlinkTarget { path, message } => {
                write!(
                    f,
                    "failed to read symlink target for {}: {message}",
                    path.display()
                )
            }
            ProgramError::HomeDirectoryUnavailable => {
                write!(f, "cannot resolve home directory from HOME")
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
        render_default_inspection_output, render_symlink_inspection_line,
    };
    use std::path::{Path, PathBuf};

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
            "invalid configuration: bad yaml"
        );
        assert_eq!(
            ProgramError::ReadConfiguration {
                path: PathBuf::from("/home/.config/gitenv/config.yml"),
                message: "missing file".to_string(),
            }
            .to_string(),
            "failed to read config at /home/.config/gitenv/config.yml: missing file"
        );
        assert_eq!(
            ProgramError::MissingEnvironment {
                vars: vec!["PRIVATE_ENV_DIR".to_string(), "OTHER".to_string()],
            }
            .to_string(),
            "missing required environment variables: PRIVATE_ENV_DIR, OTHER"
        );
        assert_eq!(
            ProgramError::IncludeNotFound {
                paths: include_paths,
            }
            .to_string(),
            "required include files not found: /includes/shared.yml, /includes/private.yml"
        );
        assert_eq!(
            ProgramError::IncludeCycle {
                cycle: include_cycle,
            }
            .to_string(),
            "include cycle detected: /includes/root.yml -> /includes/child.yml -> /includes/root.yml"
        );
        assert_eq!(
            ProgramError::ReadSourceDirectory {
                path: PathBuf::from("/repo/private"),
                message: "permission denied".to_string(),
            }
            .to_string(),
            "failed to read source directory /repo/private: permission denied"
        );
        assert_eq!(
            ProgramError::InspectPathMetadata {
                path: PathBuf::from("/home/.zshrc"),
                message: "input/output error".to_string(),
            }
            .to_string(),
            "failed to inspect path metadata for /home/.zshrc: input/output error"
        );
        assert_eq!(
            ProgramError::ReadSymlinkTarget {
                path: PathBuf::from("/home/.gitconfig"),
                message: "broken link".to_string(),
            }
            .to_string(),
            "failed to read symlink target for /home/.gitconfig: broken link"
        );
        assert_eq!(
            ProgramError::HomeDirectoryUnavailable.to_string(),
            "cannot resolve home directory from HOME"
        );
        assert_eq!(
            ProgramError::UnsupportedConfiguration.to_string(),
            "default inspection currently supports symlink operations only"
        );
    }
}
