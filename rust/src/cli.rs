use clap::{Parser, Subcommand};

use crate::{
    ApplyOperationOutcome, ApplyOperationReport, CopyInspection, CopyInspectionState,
    OperationAction, OperationInspectionOutcome, OperationPlan, ProgramError, ProgramOutput,
    SymlinkInspection, SymlinkInspectionState, inspect_operation_plan_status,
};
use std::ffi::OsString;
use std::io::IsTerminal;

const ANSI_RESET: &str = "\x1b[0m";
const ANSI_GREEN: &str = "\x1b[32m";
const ANSI_YELLOW: &str = "\x1b[33m";
const ANSI_RED: &str = "\x1b[31m";

/// Manage environment configuration files from a repository.
#[derive(Parser, Debug)]
#[command(name = "gitenv")]
#[command(about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Show the status of all configured operations (default when no command is specified).
    Info,
    /// Apply all configured operations to the system.
    Apply,
}

/// Dispatch a parsed CLI command to the appropriate library function.
///
/// Maps the selected subcommand to the appropriate library call. When no
/// subcommand is provided, the default behavior is `info`.
pub fn dispatch(cli: Cli) -> Result<ProgramOutput, ProgramError> {
    dispatch_with(cli, crate::run_info, crate::run_apply)
}

/// Like `dispatch` but accepts injectable handlers for deterministic unit tests.
///
/// Each handler is called at most once, depending on which command is selected.
pub(crate) fn dispatch_with(
    cli: Cli,
    run_info: impl FnOnce() -> Result<ProgramOutput, ProgramError>,
    run_apply: impl FnOnce() -> Result<ProgramOutput, ProgramError>,
) -> Result<ProgramOutput, ProgramError> {
    match cli.command {
        Some(Command::Info) | None => run_info(),
        Some(Command::Apply) => run_apply(),
    }
}

pub(crate) fn stdout_is_terminal() -> bool {
    std::io::stdout().is_terminal()
}

pub(crate) fn render_default_inspection_output(
    operation_plan: &OperationPlan,
    get_env_var_os: &dyn Fn(&str) -> Option<OsString>,
    stdout_is_terminal: &dyn Fn() -> bool,
) -> Result<String, ProgramError> {
    let use_color = should_use_color(get_env_var_os, stdout_is_terminal);
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

pub(crate) fn render_apply_output(
    apply_report: &ApplyOperationReport,
    get_env_var_os: &dyn Fn(&str) -> Option<OsString>,
    stdout_is_terminal: &dyn Fn() -> bool,
) -> String {
    let use_color = should_use_color(get_env_var_os, stdout_is_terminal);
    let lines = apply_report
        .outcomes
        .iter()
        .map(|outcome| render_apply_outcome_line_with_color(outcome, use_color))
        .collect::<Vec<_>>();

    if lines.is_empty() {
        "No operations to apply.".to_string()
    } else {
        lines.join("\n")
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

fn should_use_color(
    get_env_var_os: &dyn Fn(&str) -> Option<OsString>,
    stdout_is_terminal: &dyn Fn() -> bool,
) -> bool {
    stdout_is_terminal() && get_env_var_os("NO_COLOR").is_none()
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
        Cli, dispatch_with, render_apply_output, render_default_inspection_output,
        stdout_is_terminal,
    };
    use crate::{
        ApplyOperationOutcome, ApplyOperationReport, ConflictPolicy, FileOperation,
        OperationAction, OperationPlan, ProgramError, ProgramOutput,
    };
    use clap::Parser;
    use std::cell::Cell;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn tracked_ok_output<'a>(
        message: &'a str,
        calls: &'a Cell<usize>,
    ) -> impl FnOnce() -> Result<ProgramOutput, ProgramError> + 'a {
        let message = message.to_string();
        move || {
            calls.set(calls.get() + 1);
            Ok(ProgramOutput { message })
        }
    }

    #[test]
    fn invoke_the_info_command() {
        let cli = Cli::parse_from(["gitenv", "info"]);
        let info_calls = Cell::new(0);
        let apply_calls = Cell::new(0);
        let result = dispatch_with(
            cli,
            tracked_ok_output("info result", &info_calls),
            tracked_ok_output("apply result", &apply_calls),
        );
        assert_eq!(
            result,
            Ok(ProgramOutput {
                message: "info result".to_string()
            })
        );
        assert_eq!(info_calls.get(), 1);
        assert_eq!(apply_calls.get(), 0);
    }

    #[test]
    fn invoke_the_apply_command() {
        let cli = Cli::parse_from(["gitenv", "apply"]);
        let info_calls = Cell::new(0);
        let apply_calls = Cell::new(0);
        let result = dispatch_with(
            cli,
            tracked_ok_output("info result", &info_calls),
            tracked_ok_output("apply result", &apply_calls),
        );
        assert_eq!(
            result,
            Ok(ProgramOutput {
                message: "apply result".to_string()
            })
        );
        assert_eq!(info_calls.get(), 0);
        assert_eq!(apply_calls.get(), 1);
    }

    #[test]
    fn invoke_the_info_command_when_no_command_is_provided() {
        let cli = Cli::parse_from(["gitenv"]);
        let info_calls = Cell::new(0);
        let apply_calls = Cell::new(0);
        let result = dispatch_with(
            cli,
            tracked_ok_output("info result", &info_calls),
            tracked_ok_output("apply result", &apply_calls),
        );
        assert_eq!(
            result,
            Ok(ProgramOutput {
                message: "info result".to_string()
            })
        );
        assert_eq!(info_calls.get(), 1);
        assert_eq!(apply_calls.get(), 0);
    }

    #[test]
    fn render_a_missing_symlink_status_line() {
        let output = render_default_inspection_output(
            &OperationPlan {
                actions: vec![OperationAction::Symlink(FileOperation {
                    source: PathBuf::from("/repo/.gitconfig"),
                    target: PathBuf::from("/home/.gitconfig"),
                    mkdir: false,
                    conflict_policy: ConflictPolicy::Skip,
                })],
            },
            &|_: &str| None,
            &|| false,
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
            &OperationPlan {
                actions: vec![
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
                ],
            },
            &|_: &str| None,
            &|| false,
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
            &OperationPlan {
                actions: vec![
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
                ],
            },
            &|_: &str| None,
            &|| false,
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
            &OperationPlan { actions: vec![] },
            &|_: &str| None,
            &|| false,
        )
        .expect("empty operation plans should render a stable status message");

        assert_eq!(output, "No operations to inspect.");
    }

    #[test]
    fn show_no_operations_to_apply_message_when_outcomes_are_empty() {
        let report = ApplyOperationReport { outcomes: vec![] };

        let output = render_apply_output(&report, &|_: &str| None, &|| false);

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
            &|_: &str| None,
            &|| false,
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
            &|_: &str| None,
            &|| false,
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
            &|_: &str| None,
            &|| false,
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
            &OperationPlan {
                actions: vec![
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
                ],
            },
            &|_: &str| None,
            &|| true,
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
            &|_: &str| None,
            &|| true,
        );

        assert!(output.contains("\x1b[32mcreated symlink\x1b[0m"));
        assert!(output.contains("\x1b[33mskipped symlink\x1b[0m"));
        assert!(output.contains("\x1b[31munsupported: symlink\x1b[0m"));
    }

    #[test]
    fn disable_colors_when_no_color_is_set() {
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
            &|name: &str| (name == "NO_COLOR").then_some("1".into()),
            &|| true,
        );

        assert_eq!(
            output,
            "created symlink /home/.gitconfig -> /repo/.gitconfig"
        );
    }

    #[test]
    fn match_stdout_is_terminal_with_std_probe() {
        use std::io::IsTerminal;

        assert_eq!(stdout_is_terminal(), std::io::stdout().is_terminal());
    }
}
