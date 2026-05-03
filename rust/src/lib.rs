mod config;
mod intent;
mod operation;

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

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramOutput {
    pub message: &'static str,
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
    /// The current home directory is required to resolve home-relative paths.
    HomeDirectoryUnavailable,
    UnsupportedConfiguration,
}

pub fn run() -> Result<ProgramOutput, ProgramError> {
    Ok(ProgramOutput {
        message: "Hello, World!",
    })
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn show_the_default_message() {
        let output = run().expect("run should succeed");

        assert_eq!(output.message, "Hello, World!");
    }
}
