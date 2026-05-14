use clap::{Parser, Subcommand, ValueEnum};
use std::path::{Path, PathBuf};

use crate::{
    ApplyOperationOutcome, ApplyOperationReport, ColorMode, CopyInspection, CopyInspectionState,
    OperationAction, OperationInspectionOutcome, OperationPlan, PlannedOperationAction,
    ProgramError, SourceAvailability, SymlinkInspection, SymlinkInspectionState,
    inspect_operation_plan_status,
};

const ANSI_RESET: &str = "\x1b[0m";
const ANSI_GREEN: &str = "\x1b[32m";
const ANSI_YELLOW: &str = "\x1b[33m";
const ANSI_RED: &str = "\x1b[31m";
const ANSI_GRAY: &str = "\x1b[90m";

/// Display a path, replacing the home directory prefix with `~` for readability.
///
/// If the path starts with the home directory, it is displayed as `~/remainder`.
/// Otherwise, the path is displayed as-is.
fn display_path_with_home(path: &Path, home: &Path) -> String {
    if let Ok(remainder) = path.strip_prefix(home) {
        if remainder.as_os_str().is_empty() {
            "~".to_string()
        } else {
            format!("~/{}", remainder.display())
        }
    } else {
        path.display().to_string()
    }
}

/// Diagnostic log level for the `--log-level` flag.
///
/// Maps directly to `log::LevelFilter`. `off` suppresses all log output;
/// `trace` enables the most verbose output.
#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum LogLevel {
    /// Suppress all log output.
    Off,
    /// Log only errors.
    Error,
    /// Log errors and warnings (default).
    Warn,
    /// Log errors, warnings, and informational messages.
    Info,
    /// Log errors, warnings, info, and debug messages.
    Debug,
    /// Log everything, including trace-level messages.
    Trace,
}

impl From<LogLevel> for log::LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Off => log::LevelFilter::Off,
            LogLevel::Error => log::LevelFilter::Error,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Debug => log::LevelFilter::Debug,
            LogLevel::Trace => log::LevelFilter::Trace,
        }
    }
}

/// Manage environment configuration files from a repository.
#[derive(Parser, Debug)]
#[command(name = "gitenv")]
#[command(about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
    /// Path to a configuration file.
    ///
    /// CLI flag takes precedence over the `GITENV_CONFIG` environment variable.
    #[arg(
        short = 'c',
        long = "config",
        env = "GITENV_CONFIG",
        value_name = "PATH"
    )]
    pub config_path: Option<PathBuf>,
    /// Set the diagnostic log level. Log messages are written to stderr.
    #[arg(long, default_value = "warn", value_enum)]
    pub log_level: LogLevel,
    /// Control ANSI color output (`auto`, `yes`, `no`).
    ///
    /// CLI flag takes precedence over the `COLOR` environment variable.
    #[arg(long, env = "COLOR", default_value = "auto", value_enum)]
    pub color: ColorMode,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Show the status of all configured operations (default when no command is specified).
    Info,
    /// Apply all configured operations to the system.
    Apply,
}

pub(crate) fn render_default_inspection_output(
    operation_plan: &OperationPlan,
    home: &Path,
    use_color: bool,
) -> Result<String, ProgramError> {
    let inspection_report = inspect_operation_plan_status(operation_plan)?;
    let lines = inspection_report
        .outcomes
        .iter()
        .map(|outcome| render_operation_inspection_line_with_color(outcome, home, use_color))
        .collect::<Vec<_>>();

    if lines.is_empty() {
        Ok("No operations to inspect.".to_string())
    } else {
        Ok(lines.join("\n"))
    }
}

pub(crate) fn render_apply_output(
    apply_report: &ApplyOperationReport,
    home: &Path,
    use_color: bool,
) -> String {
    let lines = apply_report
        .outcomes
        .iter()
        .map(|outcome| render_apply_outcome_line_with_color(outcome, home, use_color))
        .collect::<Vec<_>>();

    if lines.is_empty() {
        "No operations to apply.".to_string()
    } else {
        lines.join("\n")
    }
}

fn render_operation_inspection_line_with_color(
    outcome: &OperationInspectionOutcome,
    home: &Path,
    use_color: bool,
) -> String {
    match outcome {
        OperationInspectionOutcome::Symlink(inspection) => {
            render_symlink_inspection_line_with_color(inspection, home, use_color)
        }
        OperationInspectionOutcome::Copy(inspection) => {
            render_copy_inspection_line_with_color(inspection, home, use_color)
        }
        OperationInspectionOutcome::Unavailable(action_entry) => {
            render_unavailable_action_line_with_color(action_entry, home, use_color)
        }
        OperationInspectionOutcome::PlanningIssue(issue) => {
            render_planning_issue_line_with_color(issue, home, use_color)
        }
    }
}

fn render_unavailable_action_line_with_color(
    action_entry: &PlannedOperationAction,
    home: &Path,
    use_color: bool,
) -> String {
    let (state, color) = if action_entry.skip_reason.is_some() {
        ("target directory is missing".to_string(), ANSI_GRAY)
    } else {
        (
            availability_state_text(&action_entry.source_availability, "source"),
            ANSI_RED,
        )
    };
    let state = colorize(&state, color, use_color);

    match &action_entry.action {
        OperationAction::Symlink(operation) => format!(
            "{} -> {}   {}",
            display_path_with_home(&operation.target, home),
            display_path_with_home(&operation.source, home),
            state,
        ),
        OperationAction::Copy(operation) => format!(
            "{} <- {}   {}",
            display_path_with_home(&operation.target, home),
            display_path_with_home(&operation.source, home),
            state,
        ),
    }
}

fn render_planning_issue_line_with_color(
    issue: &crate::OperationPlanningIssue,
    home: &Path,
    use_color: bool,
) -> String {
    format!(
        "{}   {}",
        display_path_with_home(&issue.path, home),
        colorize(
            &availability_state_text(&issue.source_availability, "source root"),
            ANSI_RED,
            use_color,
        )
    )
}

fn availability_state_text(availability: &SourceAvailability, label: &str) -> String {
    match availability {
        SourceAvailability::Available => format!("{label} is available"),
        SourceAvailability::Missing => format!("{label} is missing"),
        SourceAvailability::Unreadable { message, .. } => {
            format!("{label} is unreadable ({message})")
        }
    }
}

fn render_symlink_inspection_line_with_color(
    inspection: &SymlinkInspection,
    home: &Path,
    use_color: bool,
) -> String {
    let state = match &inspection.state {
        SymlinkInspectionState::Ok => colorize("ok", ANSI_GREEN, use_color),
        SymlinkInspectionState::Missing => colorize("not yet set up", ANSI_YELLOW, use_color),
        SymlinkInspectionState::NotASymlink => colorize("not a symlink", ANSI_RED, use_color),
        SymlinkInspectionState::PointsElsewhere { current_target } => colorize(
            &format!("points to {}", display_path_with_home(current_target, home)),
            ANSI_RED,
            use_color,
        ),
    };

    format!(
        "{} -> {}   {}",
        display_path_with_home(&inspection.target, home),
        display_path_with_home(&inspection.source, home),
        state
    )
}

fn render_copy_inspection_line_with_color(
    inspection: &CopyInspection,
    home: &Path,
    use_color: bool,
) -> String {
    let state = match inspection.state {
        CopyInspectionState::Ok => colorize("ok", ANSI_GREEN, use_color),
        CopyInspectionState::Missing => colorize("not yet set up", ANSI_YELLOW, use_color),
        CopyInspectionState::NotAFile => colorize("not a file", ANSI_RED, use_color),
        CopyInspectionState::Differs => colorize("differs from source", ANSI_RED, use_color),
    };

    format!(
        "{} <- {}   {}",
        display_path_with_home(&inspection.target, home),
        display_path_with_home(&inspection.source, home),
        state
    )
}

fn render_apply_outcome_line_with_color(
    outcome: &ApplyOperationOutcome,
    home: &Path,
    use_color: bool,
) -> String {
    match outcome {
        ApplyOperationOutcome::Applied(action) => match action {
            OperationAction::Symlink(op) => {
                format!(
                    "{} {} -> {}",
                    colorize("created symlink", ANSI_GREEN, use_color),
                    display_path_with_home(&op.target, home),
                    display_path_with_home(&op.source, home)
                )
            }
            OperationAction::Copy(op) => {
                format!(
                    "{} {} to {}",
                    colorize("copied", ANSI_GREEN, use_color),
                    display_path_with_home(&op.source, home),
                    display_path_with_home(&op.target, home)
                )
            }
        },
        ApplyOperationOutcome::SkippedExistingTarget(action) => match action {
            OperationAction::Symlink(op) => {
                format!(
                    "{} {} (already exists)",
                    colorize("skipped symlink", ANSI_YELLOW, use_color),
                    display_path_with_home(&op.target, home)
                )
            }
            OperationAction::Copy(op) => {
                format!(
                    "{} {} (already exists)",
                    colorize("skipped copy", ANSI_YELLOW, use_color),
                    display_path_with_home(&op.target, home)
                )
            }
        },
        ApplyOperationOutcome::SkippedMissingTargetDirectory(action) => match action {
            OperationAction::Symlink(op) => {
                format!(
                    "{} {} (target directory is missing)",
                    colorize("skipped symlink", ANSI_GRAY, use_color),
                    display_path_with_home(&op.target, home)
                )
            }
            OperationAction::Copy(op) => {
                format!(
                    "{} {} (target directory is missing)",
                    colorize("skipped copy", ANSI_GRAY, use_color),
                    display_path_with_home(&op.target, home)
                )
            }
        },
        ApplyOperationOutcome::UnsupportedOperation(action) => match action {
            OperationAction::Symlink(op) => {
                format!(
                    "{} {} -> {}",
                    colorize("unsupported: symlink", ANSI_RED, use_color),
                    display_path_with_home(&op.target, home),
                    display_path_with_home(&op.source, home)
                )
            }
            OperationAction::Copy(op) => {
                format!(
                    "{} {} to {}",
                    colorize("unsupported: copy", ANSI_RED, use_color),
                    display_path_with_home(&op.source, home),
                    display_path_with_home(&op.target, home)
                )
            }
        },
    }
}

fn colorize(text: &str, color: &str, use_color: bool) -> String {
    if use_color {
        format!("{color}{text}{ANSI_RESET}")
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Cli, availability_state_text, display_path_with_home, render_apply_output,
        render_default_inspection_output,
    };
    use crate::{
        ApplyOperationOutcome, ApplyOperationReport, ColorMode, ConflictPolicy, FileOperation,
        OperationAction, OperationEntry, OperationPlan, OperationPlanningIssue,
        PlannedOperationAction, SourceAvailability, SourceUnreadableKind,
    };
    use clap::{CommandFactory, Parser};
    use std::ffi::OsStr;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    const NON_MATCHING_HOME: &str = "/nonexistent/home";

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

    #[test]
    fn render_a_missing_symlink_status_line() {
        let output = render_default_inspection_output(
            &available_operation_plan(vec![OperationAction::Symlink(FileOperation {
                source: PathBuf::from("/repo/.gitconfig"),
                target: PathBuf::from("/home/.gitconfig"),
                mkdir: false,
                conflict_policy: ConflictPolicy::Skip,
            })]),
            Path::new(NON_MATCHING_HOME),
            false,
        )
        .expect("status rendering should succeed for a valid operation plan");

        assert_eq!(
            output,
            "/home/.gitconfig -> /repo/.gitconfig   not yet set up"
        );
    }

    #[test]
    fn render_operation_lines_for_each_inspection_state() {
        let output = render_default_inspection_output(
            &available_operation_plan(vec![
                OperationAction::Symlink(FileOperation {
                    source: PathBuf::from("/repo/.zshrc"),
                    target: PathBuf::from("/home/.zshrc"),
                    mkdir: false,
                    conflict_policy: ConflictPolicy::Skip,
                }),
                OperationAction::Copy(FileOperation {
                    source: PathBuf::from("/repo/.gitconfig"),
                    target: PathBuf::from("/home/.gitconfig"),
                    mkdir: false,
                    conflict_policy: ConflictPolicy::Skip,
                }),
            ]),
            Path::new(NON_MATCHING_HOME),
            false,
        )
        .expect("status rendering should support symlink and copy outcomes");

        assert_eq!(
            output,
            concat!(
                "/home/.zshrc -> /repo/.zshrc   not yet set up\n",
                "/home/.gitconfig <- /repo/.gitconfig   not yet set up"
            )
        );
    }

    #[test]
    fn render_unavailable_actions_and_planning_issues_in_order() {
        let output = render_default_inspection_output(
            &OperationPlan {
                entries: vec![
                    OperationEntry::Action(PlannedOperationAction {
                        action: OperationAction::Symlink(FileOperation {
                            source: PathBuf::from("/repo/private/.zshrc"),
                            target: PathBuf::from("/home/.zshrc"),
                            mkdir: false,
                            conflict_policy: ConflictPolicy::Skip,
                        }),
                        source_availability: SourceAvailability::Unreadable {
                            kind: SourceUnreadableKind::PermissionDenied,
                            message: "permission denied".to_string(),
                        },
                        skip_reason: None,
                    }),
                    OperationEntry::Action(PlannedOperationAction {
                        action: OperationAction::Copy(FileOperation {
                            source: PathBuf::from("/repo/private/.gitconfig"),
                            target: PathBuf::from("/home/.gitconfig"),
                            mkdir: false,
                            conflict_policy: ConflictPolicy::Skip,
                        }),
                        source_availability: SourceAvailability::Unreadable {
                            kind: SourceUnreadableKind::UnexpectedIo,
                            message: "input/output error".to_string(),
                        },
                        skip_reason: None,
                    }),
                    OperationEntry::Issue(OperationPlanningIssue {
                        path: PathBuf::from("/repo/profiles"),
                        source_availability: SourceAvailability::Missing,
                    }),
                ],
            },
            Path::new(NON_MATCHING_HOME),
            false,
        )
        .expect("status rendering should support unavailable entries");

        assert_eq!(
            output,
            concat!(
                "/home/.zshrc -> /repo/private/.zshrc   source is unreadable (permission denied)\n",
                "/home/.gitconfig <- /repo/private/.gitconfig   source is unreadable (input/output error)\n",
                "/repo/profiles   source root is missing"
            )
        );
        assert_eq!(
            availability_state_text(&SourceAvailability::Available, "source"),
            "source is available"
        );
        assert_eq!(
            availability_state_text(&SourceAvailability::Missing, "source"),
            "source is missing"
        );
        assert_eq!(
            availability_state_text(
                &SourceAvailability::Unreadable {
                    kind: SourceUnreadableKind::UnexpectedIo,
                    message: "boom".to_string(),
                },
                "source root"
            ),
            "source root is unreadable (boom)"
        );
    }

    #[test]
    fn render_recursive_entries_skipped_for_missing_target_directories() {
        let symlink_operation = FileOperation {
            source: PathBuf::from("/repo/nested/tool.conf"),
            target: PathBuf::from("/home/profiles/nested/tool.conf"),
            mkdir: true,
            conflict_policy: ConflictPolicy::Skip,
        };
        let copy_operation = FileOperation {
            source: PathBuf::from("/repo/nested/tool-copy.conf"),
            target: PathBuf::from("/home/profiles/nested/tool-copy.conf"),
            mkdir: true,
            conflict_policy: ConflictPolicy::Skip,
        };

        let info_output = render_default_inspection_output(
            &OperationPlan {
                entries: vec![OperationEntry::Action(PlannedOperationAction {
                    action: OperationAction::Symlink(symlink_operation.clone()),
                    source_availability: SourceAvailability::Available,
                    skip_reason: Some(
                        crate::operation::OperationSkipReason::MissingTargetDirectory,
                    ),
                })],
            },
            Path::new(NON_MATCHING_HOME),
            false,
        )
        .expect("status rendering should preserve skipped recursive entries");

        let apply_output = render_apply_output(
            &ApplyOperationReport {
                outcomes: vec![
                    ApplyOperationOutcome::SkippedMissingTargetDirectory(OperationAction::Symlink(
                        symlink_operation,
                    )),
                    ApplyOperationOutcome::SkippedMissingTargetDirectory(OperationAction::Copy(
                        copy_operation,
                    )),
                ],
            },
            Path::new(NON_MATCHING_HOME),
            false,
        );

        assert_eq!(
            info_output,
            "/home/profiles/nested/tool.conf -> /repo/nested/tool.conf   target directory is missing"
        );
        assert_eq!(
            apply_output,
            concat!(
                "skipped symlink /home/profiles/nested/tool.conf (target directory is missing)\n",
                "skipped copy /home/profiles/nested/tool-copy.conf (target directory is missing)"
            )
        );
    }

    #[test]
    fn render_points_elsewhere_and_copy_state_lines() {
        let temp = TempDir::new().expect("temporary directory should be created");

        let symlink_source = temp.path().join("source-symlink");
        let symlink_target = temp.path().join("target-symlink");
        let other_target = temp.path().join("other-symlink-target");
        fs::write(&symlink_source, "expected\n").expect("symlink source should be written");
        fs::write(&other_target, "different\n").expect("other target file should be written");
        symlink(&other_target, &symlink_target).expect("mismatched symlink should be created");

        let copy_source_ok = temp.path().join("copy-source-ok");
        let copy_target_ok = temp.path().join("copy-target-ok");
        fs::write(&copy_source_ok, "same\n").expect("copy ok source should be written");
        fs::write(&copy_target_ok, "same\n").expect("copy ok target should be written");

        let copy_source_differs = temp.path().join("copy-source-differs");
        let copy_target_differs = temp.path().join("copy-target-differs");
        fs::write(&copy_source_differs, "source\n").expect("copy differs source should be written");
        fs::write(&copy_target_differs, "target\n").expect("copy differs target should be written");

        let copy_source_not_a_file = temp.path().join("copy-source-not-a-file");
        let copy_target_not_a_file = temp.path().join("copy-target-not-a-file");
        fs::write(&copy_source_not_a_file, "source\n").expect("copy source should be written");
        fs::create_dir(&copy_target_not_a_file).expect("copy target directory should be created");

        let output = render_default_inspection_output(
            &available_operation_plan(vec![
                OperationAction::Symlink(FileOperation {
                    source: symlink_source.clone(),
                    target: symlink_target.clone(),
                    mkdir: false,
                    conflict_policy: ConflictPolicy::Skip,
                }),
                OperationAction::Copy(FileOperation {
                    source: copy_source_ok.clone(),
                    target: copy_target_ok,
                    mkdir: false,
                    conflict_policy: ConflictPolicy::Skip,
                }),
                OperationAction::Copy(FileOperation {
                    source: copy_source_differs,
                    target: copy_target_differs,
                    mkdir: false,
                    conflict_policy: ConflictPolicy::Skip,
                }),
                OperationAction::Copy(FileOperation {
                    source: copy_source_not_a_file,
                    target: copy_target_not_a_file,
                    mkdir: false,
                    conflict_policy: ConflictPolicy::Skip,
                }),
            ]),
            Path::new(NON_MATCHING_HOME),
            false,
        )
        .expect("status rendering should cover all state branches");

        assert!(output.contains(&format!(
            "{} -> {}   points to {}",
            symlink_target.display(),
            symlink_source.display(),
            other_target.display()
        )));
        assert!(output.contains("ok"));
        assert!(output.contains("differs from source"));
        assert!(output.contains("not a file"));
    }

    #[test]
    fn render_no_operation_message_when_nothing_is_planned() {
        let output = render_default_inspection_output(
            &available_operation_plan(vec![]),
            Path::new(NON_MATCHING_HOME),
            false,
        )
        .expect("empty operation plans should render a stable status message");

        assert_eq!(output, "No operations to inspect.");
    }

    #[test]
    fn show_no_operations_to_apply_message_when_outcomes_are_empty() {
        let report = ApplyOperationReport { outcomes: vec![] };
        let output = render_apply_output(&report, Path::new(NON_MATCHING_HOME), false);
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

        let output = render_apply_output(
            &ApplyOperationReport {
                outcomes: vec![
                    ApplyOperationOutcome::Applied(OperationAction::Symlink(symlink_op)),
                    ApplyOperationOutcome::Applied(OperationAction::Copy(copy_op)),
                ],
            },
            Path::new(NON_MATCHING_HOME),
            false,
        );

        assert_eq!(
            output,
            concat!(
                "created symlink /home/.zshrc -> /repo/.zshrc\n",
                "copied /repo/config.txt to /home/config.txt"
            )
        );
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

        let output = render_apply_output(
            &ApplyOperationReport {
                outcomes: vec![
                    ApplyOperationOutcome::SkippedExistingTarget(OperationAction::Symlink(
                        symlink_op,
                    )),
                    ApplyOperationOutcome::SkippedExistingTarget(OperationAction::Copy(copy_op)),
                ],
            },
            Path::new(NON_MATCHING_HOME),
            false,
        );

        assert_eq!(
            output,
            concat!(
                "skipped symlink /home/.zshrc (already exists)\n",
                "skipped copy /home/config.txt (already exists)"
            )
        );
    }

    #[test]
    fn show_unsupported_operation_lines_for_symlink_and_copy_actions() {
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

        let output = render_apply_output(
            &ApplyOperationReport {
                outcomes: vec![
                    ApplyOperationOutcome::UnsupportedOperation(OperationAction::Symlink(
                        symlink_op,
                    )),
                    ApplyOperationOutcome::UnsupportedOperation(OperationAction::Copy(copy_op)),
                ],
            },
            Path::new(NON_MATCHING_HOME),
            false,
        );

        assert_eq!(
            output,
            concat!(
                "unsupported: symlink /home/.zshrc -> /repo/.zshrc\n",
                "unsupported: copy /repo/config.txt to /home/config.txt"
            )
        );
    }

    #[test]
    fn render_colorized_primary_status_states() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let source_ok = temp.path().join("source-ok");
        let target_ok = temp.path().join("target-ok");
        let source_missing = temp.path().join("source-missing");
        let target_missing = temp.path().join("target-missing");
        let source_not_a_symlink = temp.path().join("source-not-a-symlink");
        let target_not_a_symlink = temp.path().join("target-not-a-symlink");

        fs::write(&source_ok, "ok\n").expect("ok source file should be written");
        symlink(&source_ok, &target_ok).expect("ok target symlink should be created");
        fs::write(&source_not_a_symlink, "expected source\n")
            .expect("source file should be written");
        fs::write(&target_not_a_symlink, "plain file\n").expect("target file should be written");

        let output = render_default_inspection_output(
            &available_operation_plan(vec![
                OperationAction::Symlink(FileOperation {
                    source: source_ok,
                    target: target_ok,
                    mkdir: false,
                    conflict_policy: ConflictPolicy::Skip,
                }),
                OperationAction::Symlink(FileOperation {
                    source: source_missing,
                    target: target_missing,
                    mkdir: false,
                    conflict_policy: ConflictPolicy::Skip,
                }),
                OperationAction::Symlink(FileOperation {
                    source: source_not_a_symlink,
                    target: target_not_a_symlink,
                    mkdir: false,
                    conflict_policy: ConflictPolicy::Skip,
                }),
            ]),
            Path::new(NON_MATCHING_HOME),
            true,
        )
        .expect("colorized status rendering should succeed");

        assert!(output.contains("\x1b[32mok\x1b[0m"));
        assert!(output.contains("\x1b[33mnot yet set up\x1b[0m"));
        assert!(output.contains("\x1b[31mnot a symlink\x1b[0m"));
    }

    #[test]
    fn render_colorized_apply_outcome_prefixes() {
        let operation = FileOperation {
            source: PathBuf::from("/repo/.gitconfig"),
            target: PathBuf::from("/home/.gitconfig"),
            mkdir: false,
            conflict_policy: ConflictPolicy::Skip,
        };

        let output = render_apply_output(
            &ApplyOperationReport {
                outcomes: vec![
                    ApplyOperationOutcome::Applied(OperationAction::Symlink(operation.clone())),
                    ApplyOperationOutcome::SkippedExistingTarget(OperationAction::Symlink(
                        operation.clone(),
                    )),
                    ApplyOperationOutcome::UnsupportedOperation(OperationAction::Symlink(
                        operation,
                    )),
                ],
            },
            Path::new(NON_MATCHING_HOME),
            true,
        );

        assert!(output.contains("\x1b[32mcreated symlink\x1b[0m"));
        assert!(output.contains("\x1b[33mskipped symlink\x1b[0m"));
        assert!(output.contains("\x1b[31munsupported: symlink\x1b[0m"));
    }

    #[test]
    fn disable_colors_when_use_color_is_false() {
        let output = render_apply_output(
            &ApplyOperationReport {
                outcomes: vec![ApplyOperationOutcome::Applied(OperationAction::Symlink(
                    FileOperation {
                        source: PathBuf::from("/repo/.gitconfig"),
                        target: PathBuf::from("/home/.gitconfig"),
                        mkdir: false,
                        conflict_policy: ConflictPolicy::Skip,
                    },
                ))],
            },
            Path::new(NON_MATCHING_HOME),
            false,
        );

        assert_eq!(
            output,
            "created symlink /home/.gitconfig -> /repo/.gitconfig"
        );
    }

    #[test]
    fn display_paths_under_home_directory_with_tilde_prefix() {
        let home = Path::new("/home/user");
        let path_under_home = Path::new("/home/user/.config/gitenv");
        let result = display_path_with_home(path_under_home, home);
        assert_eq!(result, "~/.config/gitenv");
    }

    #[test]
    fn display_paths_outside_home_directory_unchanged() {
        let home = Path::new("/home/user");
        let path_outside_home = Path::new("/etc/config");
        let result = display_path_with_home(path_outside_home, home);
        assert_eq!(result, "/etc/config");
    }

    #[test]
    fn display_paths_exactly_at_home_directory_with_tilde() {
        let home = Path::new("/home/user");
        let result = display_path_with_home(home, home);
        assert_eq!(result, "~");
    }

    #[test]
    fn do_not_replace_home_prefix_for_paths_that_only_share_a_string_prefix() {
        // /home/username2/bar shares the string prefix /home/username with /home/username,
        // but is not inside it. Path::strip_prefix() is component-aware, so it correctly
        // rejects this case and the path must be returned unchanged.
        let home = Path::new("/home/username");
        let path = Path::new("/home/username2/bar");
        let result = display_path_with_home(path, home);
        assert_eq!(result, "/home/username2/bar");
    }

    #[test]
    fn render_operation_line_with_home_relative_paths() {
        let home = Path::new("/home/user");
        let output = render_default_inspection_output(
            &available_operation_plan(vec![OperationAction::Symlink(FileOperation {
                source: PathBuf::from("/home/user/.dotfiles/.gitconfig"),
                target: PathBuf::from("/home/user/.gitconfig"),
                mkdir: false,
                conflict_policy: ConflictPolicy::Skip,
            })]),
            home,
            false,
        )
        .expect("home-relative rendering should succeed");

        assert!(output.contains("~/.gitconfig -> ~/.dotfiles/.gitconfig   not yet set up"));
    }

    // ── --log-level flag ──────────────────────────────────────────────────────

    #[test]
    fn default_log_level_is_warn() {
        let cli = Cli::parse_from(["gitenv"]);
        assert_eq!(cli.log_level, super::LogLevel::Warn);
    }

    #[test]
    fn default_color_mode_is_auto() {
        let cli = Cli::parse_from(["gitenv"]);
        assert_eq!(cli.color, ColorMode::Auto);
    }

    #[test]
    fn parse_config_path_from_the_config_flag() {
        let cli = Cli::parse_from(["gitenv", "--config", "/tmp/config.yml"]);
        assert_eq!(cli.config_path, Some(PathBuf::from("/tmp/config.yml")));
    }

    #[test]
    fn parse_config_path_from_the_short_config_flag() {
        let cli = Cli::parse_from(["gitenv", "-c", "/tmp/config.yml"]);
        assert_eq!(cli.config_path, Some(PathBuf::from("/tmp/config.yml")));
    }

    #[test]
    fn register_gitenv_config_as_the_cli_config_path_environment_variable() {
        let command = Cli::command();
        let config_path_argument = command
            .get_arguments()
            .find(|arg| arg.get_id().as_str() == "config_path")
            .expect("Cli should define a config_path argument");

        assert_eq!(
            config_path_argument.get_env(),
            Some(OsStr::new("GITENV_CONFIG"))
        );
    }

    #[test]
    fn parse_color_flag_for_each_accepted_value() {
        let cases = [
            ("auto", ColorMode::Auto),
            ("yes", ColorMode::Yes),
            ("no", ColorMode::No),
        ];

        for (raw, expected) in cases {
            let cli = Cli::parse_from(["gitenv", "--color", raw]);
            assert_eq!(
                cli.color, expected,
                "--color {raw} should parse to {expected:?}"
            );
        }
    }

    #[test]
    fn parse_log_level_flag_for_each_accepted_value() {
        use super::LogLevel;

        let cases = [
            ("off", LogLevel::Off),
            ("error", LogLevel::Error),
            ("warn", LogLevel::Warn),
            ("info", LogLevel::Info),
            ("debug", LogLevel::Debug),
            ("trace", LogLevel::Trace),
        ];

        for (raw, expected) in cases {
            let cli = Cli::parse_from(["gitenv", "--log-level", raw]);
            assert_eq!(
                cli.log_level, expected,
                "--log-level {raw} should parse to {expected:?}"
            );
        }
    }

    #[test]
    fn log_level_converts_to_level_filter() {
        use super::LogLevel;
        use log::LevelFilter;

        let cases = [
            (LogLevel::Off, LevelFilter::Off),
            (LogLevel::Error, LevelFilter::Error),
            (LogLevel::Warn, LevelFilter::Warn),
            (LogLevel::Info, LevelFilter::Info),
            (LogLevel::Debug, LevelFilter::Debug),
            (LogLevel::Trace, LevelFilter::Trace),
        ];

        for (level, expected_filter) in cases {
            let filter: LevelFilter = level.clone().into();
            assert_eq!(
                filter, expected_filter,
                "{level:?} should convert to {expected_filter:?}"
            );
        }
    }
}
