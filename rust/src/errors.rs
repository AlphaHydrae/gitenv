use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceUnreadableKind {
    PermissionDenied,
    UnexpectedIo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceAvailability {
    Available,
    Missing,
    Unreadable {
        kind: SourceUnreadableKind,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramError {
    /// Parsed configuration content is structurally invalid.
    InvalidConfiguration { message: String },
    /// A configuration file could not be read from disk.
    ConfigurationReadFailed { path: PathBuf, message: String },
    /// Required environment variables for planning are not set.
    MissingEnvironment { vars: Vec<String> },
    /// One or more required include files could not be found. The paths are
    /// sorted for deterministic output.
    IncludeNotFound { paths: Vec<PathBuf> },
    /// A config file was encountered more than once in the same include chain,
    /// forming a cycle. Contains the full cycle path in traversal order,
    /// ending with the repeated path that closes the cycle.
    IncludeCycle { cycle: Vec<PathBuf> },
    /// A source directory could not be read while expanding selectors.
    SourceDirectoryReadFailed { path: PathBuf, message: String },
    /// A filesystem path could not be inspected while deriving status or apply
    /// preconditions.
    PathInspectionFailed { path: PathBuf, message: String },
    /// A symlink target could not be read while deriving status.
    SymlinkTargetReadFailed { path: PathBuf, message: String },
    /// A symlink could not be created while applying operations.
    SymlinkCreationFailed {
        source: PathBuf,
        target: PathBuf,
        message: String,
    },
    /// A file copy operation could not be completed while applying operations.
    FileCopyFailed {
        source: PathBuf,
        target: PathBuf,
        message: String,
    },
    /// The target's parent directory could not be created before apply.
    TargetDirectoryCreationFailed { path: PathBuf, message: String },
    /// An existing target could not be removed before overwrite.
    TargetRemovalFailed { path: PathBuf, message: String },
    /// An existing target could not be moved to its backup path.
    TargetBackupFailed {
        path: PathBuf,
        backup_path: PathBuf,
        message: String,
    },
    /// Backup-on-overwrite cannot proceed because the backup path exists.
    BackupAlreadyExists { path: PathBuf },
    /// The current home directory is required to resolve home-relative paths.
    HomeDirectoryUnavailable,
    /// Apply cannot proceed because one or more planned sources are unavailable.
    OperationPlanBlocked { diagnostics: Vec<String> },
}

impl fmt::Display for ProgramError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProgramError::InvalidConfiguration { message } => {
                write!(f, "configuration is invalid ({message})")
            }
            ProgramError::ConfigurationReadFailed { path, message } => {
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
            ProgramError::SourceDirectoryReadFailed { path, message } => {
                write!(
                    f,
                    "cannot read source directory {} ({message})",
                    path.display()
                )
            }
            ProgramError::PathInspectionFailed { path, message } => {
                write!(
                    f,
                    "cannot inspect path metadata for {} ({message})",
                    path.display()
                )
            }
            ProgramError::SymlinkTargetReadFailed { path, message } => {
                write!(
                    f,
                    "cannot read symlink target for {} ({message})",
                    path.display()
                )
            }
            ProgramError::SymlinkCreationFailed {
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
            ProgramError::FileCopyFailed {
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
            ProgramError::TargetDirectoryCreationFailed { path, message } => {
                write!(
                    f,
                    "cannot create target directory {} ({message})",
                    path.display()
                )
            }
            ProgramError::TargetRemovalFailed { path, message } => {
                write!(
                    f,
                    "cannot remove existing target {} ({message})",
                    path.display()
                )
            }
            ProgramError::TargetBackupFailed {
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
            ProgramError::OperationPlanBlocked { diagnostics } => {
                write!(
                    f,
                    "cannot apply because planned sources are unavailable\n  - {}",
                    diagnostics.join("\n  - ")
                )
            }
        }
    }
}

impl std::error::Error for ProgramError {}

#[cfg(test)]
mod tests {
    use super::ProgramError;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    macro_rules! program_error_variant_patterns {
        ($m:ident) => {
            $m!(
                ProgramError::InvalidConfiguration { .. } => "InvalidConfiguration",
                ProgramError::ConfigurationReadFailed { .. } => "ConfigurationReadFailed",
                ProgramError::MissingEnvironment { .. } => "MissingEnvironment",
                ProgramError::IncludeNotFound { .. } => "IncludeNotFound",
                ProgramError::IncludeCycle { .. } => "IncludeCycle",
                ProgramError::SourceDirectoryReadFailed { .. } => "SourceDirectoryReadFailed",
                ProgramError::PathInspectionFailed { .. } => "PathInspectionFailed",
                ProgramError::SymlinkTargetReadFailed { .. } => "SymlinkTargetReadFailed",
                ProgramError::SymlinkCreationFailed { .. } => "SymlinkCreationFailed",
                ProgramError::FileCopyFailed { .. } => "FileCopyFailed",
                ProgramError::TargetDirectoryCreationFailed { .. } => "TargetDirectoryCreationFailed",
                ProgramError::TargetRemovalFailed { .. } => "TargetRemovalFailed",
                ProgramError::TargetBackupFailed { .. } => "TargetBackupFailed",
                ProgramError::BackupAlreadyExists { .. } => "BackupAlreadyExists",
                ProgramError::HomeDirectoryUnavailable => "HomeDirectoryUnavailable",
                ProgramError::OperationPlanBlocked { .. } => "OperationPlanBlocked",
            );
        };
    }

    macro_rules! define_variant_name {
        ($($pattern:pat => $name:literal),+ $(,)?) => {
            fn variant_name(error: &ProgramError) -> &'static str {
                match error {
                    $($pattern => $name,)+
                }
            }
        };
    }

    macro_rules! define_expected_variant_names {
        ($($pattern:pat => $name:literal),+ $(,)?) => {
            fn expected_variant_names() -> BTreeSet<&'static str> {
                BTreeSet::from([$($name,)+])
            }
        };
    }

    program_error_variant_patterns!(define_variant_name);
    program_error_variant_patterns!(define_expected_variant_names);

    fn stable_display_cases() -> Vec<(ProgramError, &'static str)> {
        let include_paths = vec![
            PathBuf::from("/includes/shared.yml"),
            PathBuf::from("/includes/private.yml"),
        ];
        let include_cycle = vec![
            PathBuf::from("/includes/root.yml"),
            PathBuf::from("/includes/child.yml"),
            PathBuf::from("/includes/root.yml"),
        ];

        vec![
            (
                ProgramError::InvalidConfiguration {
                    message: "bad yaml".to_string(),
                },
                "configuration is invalid (bad yaml)",
            ),
            (
                ProgramError::ConfigurationReadFailed {
                    path: PathBuf::from("/home/.config/gitenv/config.yml"),
                    message: "missing file".to_string(),
                },
                concat!(
                    "cannot read config at /home/.config/gitenv/config.yml (missing file)\n\n",
                    "Config file locations\n",
                    "  - $GITENV_CONFIG (if set)\n",
                    "  - ${XDG_CONFIG_HOME}/gitenv/config.yml (if $XDG_CONFIG_HOME is set)\n",
                    "  - ${HOME}/.config/gitenv/config.yml (default fallback)",
                ),
            ),
            (
                ProgramError::MissingEnvironment {
                    vars: vec!["PRIVATE_ENV_DIR".to_string(), "OTHER".to_string()],
                },
                "required environment variables are missing (PRIVATE_ENV_DIR, OTHER)",
            ),
            (
                ProgramError::IncludeNotFound {
                    paths: include_paths,
                },
                concat!(
                    "required include files are missing\n",
                    "  - /includes/shared.yml\n",
                    "  - /includes/private.yml"
                ),
            ),
            (
                ProgramError::IncludeCycle {
                    cycle: include_cycle,
                },
                concat!(
                    "include cycle is detected\n",
                    "  - /includes/root.yml\n",
                    "  - /includes/child.yml\n",
                    "  - /includes/root.yml"
                ),
            ),
            (
                ProgramError::SourceDirectoryReadFailed {
                    path: PathBuf::from("/repo/private"),
                    message: "permission denied".to_string(),
                },
                "cannot read source directory /repo/private (permission denied)",
            ),
            (
                ProgramError::PathInspectionFailed {
                    path: PathBuf::from("/home/.zshrc"),
                    message: "input/output error".to_string(),
                },
                "cannot inspect path metadata for /home/.zshrc (input/output error)",
            ),
            (
                ProgramError::SymlinkTargetReadFailed {
                    path: PathBuf::from("/home/.gitconfig"),
                    message: "broken link".to_string(),
                },
                "cannot read symlink target for /home/.gitconfig (broken link)",
            ),
            (
                ProgramError::SymlinkCreationFailed {
                    source: PathBuf::from("/repo/.gitconfig"),
                    target: PathBuf::from("/home/.gitconfig"),
                    message: "operation not permitted".to_string(),
                },
                "cannot create symlink /home/.gitconfig -> /repo/.gitconfig (operation not permitted)",
            ),
            (
                ProgramError::FileCopyFailed {
                    source: PathBuf::from("/repo/.gitconfig"),
                    target: PathBuf::from("/home/.gitconfig"),
                    message: "permission denied".to_string(),
                },
                "cannot copy file /repo/.gitconfig -> /home/.gitconfig (permission denied)",
            ),
            (
                ProgramError::TargetDirectoryCreationFailed {
                    path: PathBuf::from("/home/.config"),
                    message: "permission denied".to_string(),
                },
                "cannot create target directory /home/.config (permission denied)",
            ),
            (
                ProgramError::TargetRemovalFailed {
                    path: PathBuf::from("/home/.gitconfig"),
                    message: "permission denied".to_string(),
                },
                "cannot remove existing target /home/.gitconfig (permission denied)",
            ),
            (
                ProgramError::TargetBackupFailed {
                    path: PathBuf::from("/home/.gitconfig"),
                    backup_path: PathBuf::from("/home/.gitconfig.orig"),
                    message: "permission denied".to_string(),
                },
                "cannot move existing target /home/.gitconfig to backup /home/.gitconfig.orig (permission denied)",
            ),
            (
                ProgramError::BackupAlreadyExists {
                    path: PathBuf::from("/home/.gitconfig.orig"),
                },
                "cannot overwrite because backup already exists at /home/.gitconfig.orig",
            ),
            (
                ProgramError::HomeDirectoryUnavailable,
                "cannot resolve home directory from $HOME",
            ),
            (
                ProgramError::OperationPlanBlocked {
                    diagnostics: vec![
                        "source /repo/private/.secret for /home/.secret is unreadable (permission denied)".to_string(),
                        "source root /repo/profiles is unreadable (input/output error)".to_string(),
                    ],
                },
                concat!(
                    "cannot apply because planned sources are unavailable\n",
                    "  - source /repo/private/.secret for /home/.secret is unreadable (permission denied)\n",
                    "  - source root /repo/profiles is unreadable (input/output error)"
                ),
            ),
        ]
    }

    #[test]
    fn format_program_errors_with_stable_messages() {
        let cases = stable_display_cases();
        let observed_variant_names: BTreeSet<_> =
            cases.iter().map(|(error, _)| variant_name(error)).collect();

        assert_eq!(
            observed_variant_names,
            expected_variant_names(),
            "stable_display_cases must include exactly one case for every ProgramError variant",
        );
        assert_eq!(
            cases.len(),
            observed_variant_names.len(),
            "stable_display_cases must not contain duplicate ProgramError variants",
        );

        for (error, expected) in cases {
            let name = variant_name(&error);
            assert_eq!(
                error.to_string(),
                expected,
                "unexpected display message for {}",
                name,
            );
        }
    }
}
