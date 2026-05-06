use clap::{Parser, Subcommand};

use crate::{ProgramError, ProgramOutput};

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

#[cfg(test)]
mod tests {
    use super::{Cli, dispatch_with};
    use crate::{ProgramError, ProgramOutput};
    use clap::Parser;
    use std::cell::Cell;

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
}
